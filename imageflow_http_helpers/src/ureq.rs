
use std;
use ureq;
use std::io::Read;
use std::str;
use crate::{HttpFetcher, FetchError, FetchResult, FetchedResponse, FetchConfig};

impl From<ureq::Error> for FetchError {
    fn from(e: ureq::Error) -> FetchError {
        FetchError::ToolError(format!("ureq error: {:?}", e))
    }
}


pub struct UreqFetcher;

impl UreqFetcher {
    pub fn new() -> Self {
        UreqFetcher
    }
}

impl HttpFetcher for UreqFetcher {
    fn fetch(&self, url: &str, config: Option<FetchConfig>) -> FetchResult {
        let conf = config.unwrap_or_default();

        let read_error_body = conf.read_error_body.unwrap_or(false);
        let ureq_config = ureq::config::Config::builder().http_status_as_error(read_error_body).build();


        let agent = ureq_config.new_agent();


        match agent.get(url).call() {
            Ok(response) => {
                let status = response.status();
                let success = status.is_success();
                let content_length = response
                    .headers()
                    .get("Content-Length")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<usize>().ok());

                let content_type = response
                    .headers()
                    .get("Content-Type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("application/octet-stream")
                    .to_string();

                let mut bytes: Vec<u8> = Vec::new();
                if let Some(len) = content_length {
                    bytes.reserve(len);
                }

                let mut reader = response.into_body().into_reader();
                reader.read_to_end(&mut bytes)?;

                if content_length.is_some() && content_length.unwrap() != bytes.len() {
                    return Err(FetchError::ContentLengthMismatch);
                }

                if read_error_body && !success{
                    return Err(FetchError::UpstreamResponseErrorWithResponse {
                        status: status.as_u16(),
                        response: FetchedResponse {
                            code: status.as_u16(),
                            content_type,
                            bytes,
                        },
                    });
                }

                Ok(FetchedResponse {
                    code: status.as_u16(),
                    content_type,
                    bytes,
                })
            }
            Err(e) => match e {
                ureq::Error::StatusCode(code) => {
                    Err(FetchError::UpstreamResponseError(code))
                }
                e => Err(e.into()),
            }
        }
    }

    fn get_status_code(&self, url: &str) -> std::result::Result<u16, FetchError> {
        let agent = ureq::agent();

        match agent.get(url).call() {
            Ok(response) => Ok(response.status().as_u16()),
            Err(ureq::Error::StatusCode(code)) => Ok(code),
            Err(e) => Err(e.into())
        }
    }
}
