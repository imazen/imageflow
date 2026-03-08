use crate::fc;
use crate::s;

use crate::fc::errors::CategorizedError;
use crate::fc::{ErrorCategory, JsonResponse};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

pub enum JobSource {
    JsonFile(PathBuf),
    // NamedDemo(String),
    Ir4QueryString(String),
}

// CmdBuild
// --bundle-into folder
// Copies physical files referenced into 'folder'
// Copies recipe into folder (after transforming)
// Fetches remote URLs into folder
// Fetches remote paths in b

// CmdBuild
// --debug-?
// Export transformed .json recipe post-injection
//

//pub struct CmdProxy {
//    invocation_args: Args,
//
//    let m: &&clap::ArgMatches = matches;
//
//
//    let source = if m.is_present("demo") {
//    cmd_build::JobSource::NamedDemo(m.value_of("demo").unwrap().to_owned())
//    } else {
//    cmd_build::JobSource::JsonFile(m.value_of("json").unwrap().to_owned())
//    };
//
//    let builder =
//    cmd_build::CmdBuild::parse(source, m.values_of_lossy("in"), m.values_of_lossy("out"))
//    .build_maybe();
//    builder.write_response_maybe(m.value_of("response"))
//    .expect("IO error writing JSON output file. Does the directory exist?");
//    builder.write_errors_maybe().expect("Writing to stderr failed!");
//    return builder.get_exit_code().unwrap();
//}

pub struct CmdBuild {
    job: Result<s::Build001>,
    response: Option<Result<s::ResponsePayload>>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum CmdError {
    // DemoNotFound(String),
    JsonRecipeNotFound(String),
    IoError(std::io::Error),
    InvalidJson(serde_json::error::Error),
    IoIdNotInRecipe(i32),
    BadArguments(String),
    InconsistentUseOfIoId(String),
    // NotImplemented,
    FlowError(fc::FlowError),
    Incomplete,
}
impl CategorizedError for CmdError {
    fn category(&self) -> ErrorCategory {
        match *self{
            CmdError::JsonRecipeNotFound(_) |
           // CmdError::DemoNotFound(_) => ErrorCategory::PrimaryResourceNotFound,
            CmdError::IoError(_) => ErrorCategory::IoError,
            CmdError::BadArguments(_)|
            CmdError::InconsistentUseOfIoId(_) |
            CmdError::IoIdNotInRecipe(_) => ErrorCategory::ArgumentInvalid,
            CmdError::InvalidJson(_) => ErrorCategory::InvalidJson,
            CmdError::Incomplete => ErrorCategory::InternalError,
            CmdError::FlowError(ref fe) => fe.category()
        }
    }
}
impl CmdError {
    pub fn to_json(&self) -> JsonResponse {
        let message = format!("{:#?}", self);
        JsonResponse::fail_with_message(i64::from(self.category().http_status_code()), &message)
    }
    pub fn exit_code(&self) -> i32 {
        self.category().process_exit_code()
    }
}

pub type Result<T> = std::result::Result<T, CmdError>;

impl From<std::io::Error> for CmdError {
    fn from(e: std::io::Error) -> CmdError {
        CmdError::IoError(e)
    }
}
impl From<serde_json::error::Error> for CmdError {
    fn from(e: serde_json::error::Error) -> CmdError {
        CmdError::InvalidJson(e)
    }
}
impl From<fc::FlowError> for CmdError {
    fn from(e: fc::FlowError) -> CmdError {
        CmdError::FlowError(e)
    }
}

impl std::fmt::Display for CmdError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let CmdError::FlowError(ref e) = *self {
            write!(f, "{}", e)
        } else {
            write!(f, "{:#?}", self)
        }
    }
}

fn parse_io_enum(s: &str) -> s::IoEnum {
    match s {
        "base64:" => s::IoEnum::OutputBase64,
        s if s.starts_with("http://") || s.starts_with("https://") => {
            panic!("URLs are not permitted")
        }
        s => s::IoEnum::Filename(s.to_owned()),
    }
}

impl CmdBuild {
    fn load_job(source: JobSource) -> Result<s::Build001> {
        match source {
            JobSource::JsonFile(path) => {
                let mut data = Vec::new();
                let mut f = match File::open(&path) {
                    Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                        return Err(CmdError::JsonRecipeNotFound(path.to_string_lossy().into()));
                    }
                    other => other,
                }?;
                f.read_to_end(&mut data)?;

                let parsed: s::Build001 = serde_json::from_slice(&data)?;
                Ok(parsed)
            }
            JobSource::Ir4QueryString(query) => {
                let build = s::Build001 {
                    builder_config: None,
                    io: vec![
                        s::IoObject {
                            io_id: 0,
                            direction: s::IoDirection::In,
                            io: s::IoEnum::Placeholder,
                        },
                        s::IoObject {
                            io_id: 1,
                            direction: s::IoDirection::Out,
                            io: s::IoEnum::Placeholder,
                        },
                    ],
                    framewise: s::Framewise::Steps(vec![s::Node::CommandString {
                        decode: Some(0),
                        encode: Some(1),
                        kind: s::CommandStringKind::ImageResizer4,
                        value: query,
                        watermarks: None,
                    }]),
                };
                Ok(build)
            } // JobSource::NamedDemo(name) => Err(CmdError::DemoNotFound(name)),
        }
    }

    fn inject(
        before: s::Build001,
        inject: Option<Vec<String>>,
        dir: s::IoDirection,
    ) -> Result<s::Build001> {
        if inject.is_none() {
            return Ok(before);
        }
        let args: Vec<String> = inject.unwrap();
        if args.is_empty() {
            return Ok(before);
        }

        let user_providing_numbers = args.as_slice().iter().any(|v| v.parse::<i32>().is_ok());

        // If they're not all consecutive, we can't assign by io_id. Let's not even try. Just do order of appearance.
        // /old_io_ids.sort(); //ascending
        // /let all_consecutive = old_io_ids.as_slice().iter().fold(Some(old_io_ids[0] -1),|prev, current| if let Some(p) = prev && current == p + 1 { Some(current)} else {None}  ).is_some();
        // /let first_existing_io_id = old_io_ids.as_slice().iter().min().unwrap();
        let old_io_ids = before
            .io
            .as_slice()
            .iter()
            .filter(|io| io.direction == dir)
            .map(|io| io.io_id)
            .collect::<Vec<i32>>();

        let max_possible_args = if user_providing_numbers { args.len() / 2 } else { args.len() };

        if max_possible_args > old_io_ids.len() {
            return Err(CmdError::BadArguments(format!(
                "Too many arguments provided for {:?}. Only {} openings in the recipe ({:?}).",
                dir,
                old_io_ids.len(),
                &old_io_ids
            )));
        }

        let vec_of_io_results = if user_providing_numbers {
            args.as_slice().chunks(2).map(|pair| {
                if pair.len() == 1 {
                    return Err(CmdError::InconsistentUseOfIoId(
                        format!("Use of io_id values must be consistent. Odd number of values ({}) for {:?}", args.len(), dir)));
                }
                let io_id = match pair[0].parse::<i32>() {
                    Ok(v) if old_io_ids.contains(&v) => v,
                    Ok(v) => {
                        return Err(CmdError::IoIdNotInRecipe(v));
                    }
                    Err(_) => {
                        return Err(CmdError::InconsistentUseOfIoId(
                            format!("Expected numeric io_id, found {}. Use io_ids consistently or allow implicit numbering by order of IoObject appearance in the json file", pair[0])));
                    }
                };
                Ok((io_id, pair[1].as_ref()))
            }).collect::<Vec<Result<(i32,&str)>>>()
        } else {
            args.as_slice()
                .iter()
                .enumerate()
                .map(|(index, v)| Ok((old_io_ids[index], v.as_ref())))
                .collect::<Vec<Result<(i32, &str)>>>()
        };

        let mut hash = HashMap::new();
        for item in vec_of_io_results {
            let (k, v) = item?;

            if let Some(old_value) = hash.insert(k, v) {
                return Err(CmdError::BadArguments(format!(
                    "Duplicate values for io_id {}: {} and {}",
                    k, old_value, v
                )));
            }
        }

        let old_io_copy = before.io.clone();

        Ok(s::Build001 {
            io: old_io_copy
                .into_iter()
                .map(|io| {
                    let id = io.io_id;
                    if let Some(v) = hash.get(&id) {
                        s::IoObject { direction: dir, io_id: id, io: parse_io_enum(v) }
                    } else {
                        io
                    }
                })
                .collect::<Vec<s::IoObject>>(),
            ..before
        })
    }
    fn inject_security(
        before: s::Build001,
        limit_args: Option<Vec<String>>,
    ) -> Result<s::Build001> {
        if limit_args.is_none() {
            return Ok(before);
        }
        let lowercase_args =
            limit_args.unwrap().into_iter().map(|s| s.to_lowercase()).collect::<Vec<String>>();
        let args_string = lowercase_args.join(" ");
        let security;
        let unlimited_frame_size =
            s::FrameSizeLimit { w: i32::MAX as u32, h: i32::MAX as u32, megapixels: f32::MAX };
        if lowercase_args.contains(&"disabled".to_string()) {
            security = Some(s::ExecutionSecurity {
                max_decode_size: Some(unlimited_frame_size),
                max_frame_size: Some(unlimited_frame_size),
                max_encode_size: Some(unlimited_frame_size),
            })
        } else {
            let mut max_frame_size = unlimited_frame_size;
            let mut found_arg = false;
            for arg in lowercase_args {
                // parse [number](px|w|h|mp) and match to ExecutionSecury members
                // parse all non-numeric chars from end and match on them.
                if arg.ends_with("px") {
                    let number = arg.split_at(arg.len() - 2).0.parse::<u32>().map_err(|_| {
                        CmdError::BadArguments(format!(
                            "Invalid number in argument to --exit-if-larger-than: {} ({})",
                            arg,
                            arg.split_at(arg.len() - 2).0
                        ))
                    })?;
                    max_frame_size = s::FrameSizeLimit { w: number, h: number, ..max_frame_size };
                } else if arg.ends_with("w") {
                    let number = arg.split_at(arg.len() - 1).0.parse::<u32>().map_err(|_| {
                        CmdError::BadArguments(format!(
                            "Invalid number in argument to --exit-if-larger-than: {}",
                            arg
                        ))
                    })?;
                    max_frame_size = s::FrameSizeLimit { w: number, ..max_frame_size };
                } else if arg.ends_with("h") {
                    let number = arg.split_at(arg.len() - 1).0.parse::<u32>().map_err(|_| {
                        CmdError::BadArguments(format!(
                            "Invalid number in argument to --exit-if-larger-than: {}",
                            arg
                        ))
                    })?;
                    max_frame_size = s::FrameSizeLimit { h: number, ..max_frame_size };
                } else if arg.ends_with("mp") {
                    let number = arg.split_at(arg.len() - 2).0.parse::<f32>().map_err(|_| {
                        CmdError::BadArguments(format!(
                            "Invalid number in argument to --exit-if-larger-than: {}",
                            arg
                        ))
                    })?;
                    max_frame_size = s::FrameSizeLimit { megapixels: number, ..max_frame_size };
                } else {
                    let number = arg.parse::<u32>().map_err(|_| {
                        CmdError::BadArguments(format!(
                            "Invalid number as argument to --exit-if-larger-than: {}",
                            arg
                        ))
                    })?;
                    max_frame_size = s::FrameSizeLimit { w: number, h: number, ..max_frame_size };
                }
                found_arg = true;
            }
            if !found_arg {
                return Err(CmdError::BadArguments(format!(
                    "Invalid arguments to --exit-if-larger-than: {}",
                    args_string
                )));
            }
            security = Some(s::ExecutionSecurity {
                max_frame_size: Some(max_frame_size),
                ..s::ExecutionSecurity::sane_defaults()
            });
        }

        let builder_config =
            s::Build001Config { security, ..before.builder_config.clone().unwrap_or_default() };
        Ok(s::Build001 { builder_config: Some(builder_config), ..before })
    }
    fn parse_maybe(
        source: JobSource,
        in_args: Option<Vec<String>>,
        out_args: Option<Vec<String>>,
        limit_args: Option<Vec<String>>,
    ) -> Result<s::Build001> {
        let original = CmdBuild::load_job(source)?;
        let a = CmdBuild::inject(original, in_args, s::IoDirection::In)?;
        let b = CmdBuild::inject_security(a, limit_args)?;
        CmdBuild::inject(b, out_args, s::IoDirection::Out)
    }
    pub fn parse(
        source: JobSource,
        in_args: Option<Vec<String>>,
        out_args: Option<Vec<String>>,
        limit_args: Option<Vec<String>>,
    ) -> CmdBuild {
        CmdBuild {
            job: CmdBuild::parse_maybe(source, in_args, out_args, limit_args),
            response: None,
        }
    }

    fn transform_build(b: s::Build001, directory: &Path) -> Result<(Vec<String>, s::Build001)> {
        let mut log = Vec::new();
        let transformed = b
            .io
            .into_iter()
            .map(|obj| {
                let e: s::IoEnum = obj.io;
                let new_enum = if obj.direction == s::IoDirection::In {
                    match e {
                        s::IoEnum::Filename(path) => {
                            let fname = format!(
                                "input_{}_{}",
                                obj.io_id,
                                std::path::Path::new(&path).file_name().unwrap().to_str().unwrap()
                            );
                            let new_path =
                                directory.join(&fname).as_os_str().to_str().unwrap().to_owned();
                            std::fs::copy(&path, &new_path).unwrap();
                            log.push(format!(
                                "Copied {} to {} (referenced as {})",
                                &path, &new_path, &fname
                            ));
                            s::IoEnum::Filename(fname)
                        }
                        // s::IoEnum::Url(url) => {
                        //
                        //     let fname = format!("input_{}", obj.io_id);
                        //     let new_path = directory.join(&fname).as_os_str().to_str().unwrap().to_owned();
                        //     let bytes = ::imageflow_http_helpers::fetch_bytes(&url).unwrap();
                        //     let mut file = BufWriter::new(File::create(&new_path).unwrap());
                        //     file.write_all(&bytes).unwrap();
                        //     log.push(format!("Downloaded {} to {} (referenced as {})", &url, &new_path, &fname));
                        //     s::IoEnum::Filename(fname)
                        // }
                        other => other,
                    }
                } else {
                    match e {
                        s::IoEnum::Filename(path) => {
                            let fname = format!(
                                "output_{}_{}",
                                obj.io_id,
                                &std::path::Path::new(&path).file_name().unwrap().to_str().unwrap()
                            );
                            //let new_path = directory.join(&fname).as_os_str().to_str().unwrap().to_owned();
                            log.push(format!("Changed output {} to {}", &path, &fname));
                            s::IoEnum::Filename(fname)
                        }
                        other => other,
                    }
                };
                s::IoObject { direction: obj.direction, io: new_enum, io_id: obj.io_id }
            })
            .collect::<Vec<s::IoObject>>();
        Ok((
            log,
            s::Build001 {
                io: transformed,
                builder_config: b.builder_config,
                framewise: b.framewise,
            },
        ))
    }
    fn write_json<T, P: AsRef<Path>>(path: &P, info: &T)
    where
        T: serde::Serialize,
    {
        let mut file = BufWriter::new(File::create(path).unwrap());
        write!(file, "{}", serde_json::to_string_pretty(info).unwrap()).unwrap();
    }

    // Write new invocation to STDOUT, for execution in 'directory'.
    // Will write recipe and dependencies into directory
    pub fn bundle_to(self, directory: &Path) -> i32 {
        match std::fs::create_dir(directory) {
            Ok(_) => (),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    eprintln!("Error: --bundle-to target directory already exists: {}", e);
                    return 1;
                } else {
                    eprintln!("Failed to create directory: {}", e);
                    return 1;
                }
            }
        }
        let (log, transformed) = CmdBuild::transform_build(self.job.unwrap(), directory).unwrap();
        CmdBuild::write_json(&directory.join("recipe.json"), &transformed);
        println!("cd {:?}", &directory);
        println!("imageflow_tool --json recipe.json\n\n");
        for s in log {
            println!("# {}", &s);
        }
        0
    }

    pub fn build_maybe(self) -> CmdBuild {
        let response = if let Ok(ref b) = self.job {
            CmdBuild::build(b.clone())
        } else {
            Err(CmdError::Incomplete)
        };

        CmdBuild { response: Some(response), ..self }
    }

    pub fn get_json_response(&self) -> JsonResponse {
        if let Some(e) = self.get_first_error() {
            e.to_json()
        } else if let Some(Ok(ref r)) = self.response {
            JsonResponse::from_result(Ok(r.clone()))
        } else {
            // Should not be called before maybe_build
            unreachable!();
        }
    }
    ///
    /// Write the JSON response (if present) to the given file or STDOUT
    pub fn write_response_maybe(
        &self,
        response_file: Option<&PathBuf>,
        allow_stdout: bool,
    ) -> std::io::Result<()> {
        if self.response.is_some() {
            if let Some(filename) = response_file {
                let mut file = BufWriter::new(File::create(filename).unwrap());
                file.write_all(&self.get_json_response().response_json)?;
            } else if allow_stdout {
                std::io::stdout().write_all(&self.get_json_response().response_json)?;
            }
        }
        Ok(())
    }

    pub fn write_errors_maybe(&self) -> std::io::Result<()> {
        if let Some(e) = self.get_first_error() {
            writeln!(&mut std::io::stderr(), "{}", e)?;
        }
        Ok(())
    }

    pub fn get_first_error(&self) -> Option<&CmdError> {
        if let Err(ref e) = self.job {
            return Some(e);
        }
        match self.response {
            Some(Err(ref e)) => Some(e),
            _ => None,
        }
    }

    pub fn get_exit_code(&self) -> Option<i32> {
        self.get_first_error().map(|e| e.exit_code()).or(if self.response.is_some() {
            Some(0)
        } else {
            None
        })
    }

    fn build(data: s::Build001) -> Result<s::ResponsePayload> {
        let mut context = fc::Context::create()?;
        Ok(context.build_1(data)?)
    }
}
