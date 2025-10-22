use crate::{FetchConfig, FetchError, FetchResult, FetchedResponse, HttpFetcher};
use std::env;
use std::io;
use std::process::Command;

pub struct ShellFetcher;

impl Default for ShellFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellFetcher {
    pub fn new() -> Self {
        ShellFetcher
    }

    fn detect_tool() -> Option<(&'static str, Vec<String>)> {
        // Try curl first
        if Command::new("curl").arg("--version").output().is_ok() {
            return Some((
                "curl",
                vec![
                    "-L".to_string(), // Follow redirects
                    "-s".to_string(), // Silent mode
                    "-f".to_string(), // Fail on HTTP errors
                    "-o".to_string(), // Output to file
                ],
            ));
        }

        // Try wget as fallback
        if Command::new("wget").arg("--version").output().is_ok() {
            return Some((
                "wget",
                vec![
                    "-q".to_string(), // Quiet mode
                    "--no-check-certificate".to_string(),
                    "-O".to_string(), // Output to file
                ],
            ));
        }

        // On Windows, try PowerShell
        if cfg!(windows) && Command::new("powershell.exe").arg("-Help").output().is_ok() {
            return Some((
                "powershell.exe",
                vec![
                    "-Command".to_string(),
                    "Invoke-WebRequest".to_string(),
                    "-OutFile".to_string(),
                ],
            ));
        }

        None
    }
}

impl HttpFetcher for ShellFetcher {
    fn fetch(&self, url: &str, conf: Option<FetchConfig>) -> FetchResult {
        let conf = conf.unwrap_or_default();
        let read_error_body = conf.read_error_body.unwrap_or(false);

        let (tool, base_args) = Self::detect_tool().ok_or_else(|| {
            FetchError::IoError(io::Error::new(
                io::ErrorKind::NotFound,
                "No suitable download tool found (curl, wget, or powershell)",
            ))
        })?;

        // Create temporary files for output and errors
        let temp_dir = env::temp_dir();
        let temp_file = temp_dir.join(format!("download_{}", uuid::Uuid::new_v4()));
        let error_file = temp_dir.join(format!("error_{}", uuid::Uuid::new_v4()));
        let temp_path = temp_file.to_str().unwrap();
        let error_path = error_file.to_str().unwrap();

        // Prepare command based on the tool
        let output = match tool {
            "curl" => {
                let mut args = base_args;
                if read_error_body {
                    args.retain(|arg| arg != "-f");
                }
                args.push(temp_path.to_string());
                args.push(url.to_string());

                Command::new(tool)
                    .args(&args)
                    .stderr(std::fs::File::create(error_path)?)
                    .output()?
            }
            "wget" => {
                let mut args = base_args;
                args.push(temp_path.to_string());
                args.push(url.to_string());

                Command::new(tool)
                    .args(&args)
                    .stderr(std::fs::File::create(error_path)?)
                    .output()?
            }
            "powershell.exe" => {
                // Simpler PowerShell approach that works with older versions
                let args = vec![
                    "-Command".to_string(),
                    format!("try {{ (New-Object System.Net.WebClient).DownloadFile('{}', '{}') }} catch {{ $_.Exception.Response.StatusCode.Value__ }}",
                        url, temp_path)
                ];

                Command::new(tool)
                    .args(&args)
                    .stderr(std::fs::File::create(error_path)?)
                    .output()?
            }
            _ => unreachable!(),
        };

        // Read the downloaded file and error output
        let bytes =
            if std::fs::metadata(temp_path).is_ok() { std::fs::read(temp_path)? } else { vec![] };

        let error_bytes =
            if std::fs::metadata(error_path).is_ok() { std::fs::read(error_path)? } else { vec![] };

        // Clean up
        let _ = std::fs::remove_file(temp_path);
        let _ = std::fs::remove_file(error_path);

        // Handle the response
        if !output.status.success() {
            if read_error_body {
                // Use error output if main output is empty
                let response_bytes = if bytes.is_empty() { error_bytes } else { bytes };
                if !response_bytes.is_empty() {
                    return Ok(FetchedResponse {
                        code: 500,
                        bytes: response_bytes,
                        content_type: "application/octet-stream".to_string(),
                    });
                }
            }
            return Err(FetchError::UpstreamResponseError(500));
        }

        Ok(FetchedResponse {
            code: 200,
            bytes,
            content_type: "application/octet-stream".to_string(),
        })
    }

    fn get_status_code(&self, url: &str) -> std::result::Result<u16, FetchError> {
        let (tool, _) = Self::detect_tool().ok_or_else(|| {
            FetchError::IoError(io::Error::new(
                io::ErrorKind::NotFound,
                "No suitable download tool found (curl, wget, or powershell)",
            ))
        })?;

        // For curl, we can use -I to get headers only
        let status = match tool {
            "curl" => {
                let mut args = vec![
                    "-I".to_string(),
                    "-s".to_string(),
                    "-o".to_string(),
                    "/dev/null".to_string(),
                ];
                args.push(url.to_string());
                let status = Command::new(tool).args(&args).status()?;
                // HEAD is not supported by many servers, so if not successful, fall back to self.fetch
                if !status.success() {
                    return self.fetch(url, None).map(|r| r.code);
                }
                status
            }
            // For wget and powershell, just do a HEAD request
            _ => {
                // Fallback to actual fetch since these tools don't have great header-only options
                return self.fetch(url, None).map(|r| r.code);
            }
        };

        if status.success() {
            Ok(200)
        } else {
            Ok(404) // This is a simplification - shell tools don't easily expose HTTP status codes
        }
    }
}
