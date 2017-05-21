use fc;
use s;
use serde_json;
use std;
extern crate core;
extern crate serde;
// use self::core::slice::SliceExt;

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path};
use std::io::{Write, Read, BufWriter};

pub enum JobSource {
    JsonFile(String),
    NamedDemo(String),
    Ir4QueryString(String)
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
    response: Option<Result<fc::JsonResponse>>,
}

#[derive(Debug)]
pub enum CmdError {
    DemoNotFound(String),
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

impl CmdError {
    pub fn exit_code(&self) -> i32 {
        //        #define EX_USAGE	64	/* command line usage error */
        //        #define EX_DATAERR	65	/* data format error */
        //        #define EX_NOINPUT	66	/* cannot open input */
        //        #define EX_NOUSER	67	/* addressee unknown */
        //        #define EX_NOHOST	68	/* host name unknown */
        //        #define EX_UNAVAILABLE	69	/* service unavailable */
        //        #define EX_SOFTWARE	70	/* internal software error */
        //        #define EX_OSERR	71	/* system error (e.g., can't fork) */
        //        #define EX_OSFILE	72	/* critical OS file missing */
        //        #define EX_CANTCREAT	73	/* can't create (user) output file */
        //        #define EX_IOERR	74	/* input/output error */
        //        #define EX_TEMPFAIL	75	/* temp failure; user is invited to retry */
        //        #define EX_PROTOCOL	76	/* remote error in protocol */
        //        #define EX_NOPERM	77	/* permission denied */
        //        #define EX_CONFIG	78	/* configuration error */

        match *self {
            CmdError::JsonRecipeNotFound(_) |
            CmdError::DemoNotFound(_) => 66,
            CmdError::IoError(_) => 74,
            CmdError::BadArguments(_) => 64,
            CmdError::InconsistentUseOfIoId(_) => 64,
            CmdError::IoIdNotInRecipe(_) => 64,
            CmdError::InvalidJson(_) => 65,
            CmdError::Incomplete => 70, //also CmdError::NotImplemented if we bring it back
            CmdError::FlowError(ref fe) => {
                match *fe {
                    fc::FlowError::Oom => 71,
                    fc::FlowError::Err(ref flow_err) => {
                        match flow_err.code {
                            10 => 71,
                            20 => 74,
                            // 60 => 65, //image decoding failed
                            _ => 70,
                        }
                    }
                    _ => 70,
                }
            }
        }
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


fn parse_io_enum(s: &str) -> s::IoEnum {
    match s {
        "base64:" => s::IoEnum::OutputBase64,
        s if s.starts_with("http://") || s.starts_with("https://") => s::IoEnum::Url(s.to_owned()),
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
                        return Err(CmdError::JsonRecipeNotFound(path.to_owned()));
                    }
                    other => other,
                }?;
                f.read_to_end(&mut data)?;

                let parsed: s::Build001 = serde_json::from_slice(&data)?;
                Ok(parsed)
            }
            JobSource::Ir4QueryString(query) => {
                let build = s::Build001{
                    builder_config: None,
                    io: vec![
                        s::IoObject{
                            io_id: 0,
                            direction: s::IoDirection::In,
                            io: s::IoEnum::Placeholder
                        },
                        s::IoObject{
                            io_id: 1,
                            direction: s::IoDirection::Out,
                            io: s::IoEnum::Placeholder
                        }
                    ],
                    framewise: s::Framewise::Steps(vec![
                        s::Node::CommandString {
                        decode: Some(0),
                        encode: Some(1),
                        kind: s::CommandStringKind::ImageResizer4,
                        value: query
                    }])
                };
                Ok(build)
            }
            JobSource::NamedDemo(name) => Err(CmdError::DemoNotFound(name)),
        }
    }


    fn inject(before: s::Build001,
              inject: Option<Vec<String>>,
              dir: s::IoDirection)
              -> Result<s::Build001> {
        if inject.is_none() {
            return Ok(before);
        }
        let args: Vec<String> = inject.unwrap();
        if args.len() == 0 {
            return Ok(before);
        }

        let user_providing_numbers = args.as_slice().iter().any(|v| v.parse::<i32>().is_ok());

        // If they're not all consecutive, we can't assign by io_id. Let's not even try. Just do order of appearance.
        // /old_io_ids.sort(); //ascending
        // /let all_consecutive = old_io_ids.as_slice().iter().fold(Some(old_io_ids[0] -1),|prev, current| if let Some(p) = prev && current == p + 1 { Some(current)} else {None}  ).is_some();
        // /let first_existing_io_id = old_io_ids.as_slice().iter().min().unwrap();
        let old_io_ids = before.io
            .as_slice()
            .iter()
            .filter(|io| io.direction == dir)
            .map(|io| io.io_id)
            .collect::<Vec<i32>>();



        let max_possible_args = match user_providing_numbers {
            true => args.len() / 2,
            false => args.len(),
        };
        if max_possible_args > old_io_ids.len() {
            return Err(CmdError::BadArguments(format!("Too many arguments provided for {:?}. Only {} openings in the recipe ({:?}).",
                                                      dir,
                                                      old_io_ids.len(),
                                                      &old_io_ids)));
        }

        let vec_of_io_results = match user_providing_numbers {
            true => {
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
            }
            false => {
                args.as_slice()
                    .iter()
                    .enumerate()
                    .map(|(index, v)| Ok((old_io_ids[index], v.as_ref())))
                    .collect::<Vec<Result<(i32, &str)>>>()
            }
        };


        let mut hash = HashMap::new();
        for item in vec_of_io_results.into_iter() {
            match item {
                Ok((k, v)) => {
                    match hash.insert(k, v) {
                        Some(ref old_value) => {
                            return Err(CmdError::BadArguments(format!("Duplicate values for io_id {}: {} and {}",
                                                                      k,
                                                                      old_value,
                                                                      v)));
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }

        let old_io_copy = before.io.clone();

        Ok(s::Build001 {
            io: old_io_copy.into_iter()
                .map(|io| {
                    let id = io.io_id;
                    if let Some(v) = hash.get(&id) {
                        s::IoObject {
                            direction: dir,
                            io_id: id,
                            io: parse_io_enum(v),
                        }
                    } else {
                        io
                    }
                })
                .collect::<Vec<s::IoObject>>(),
            ..before
        })
    }

    fn parse_maybe(source: JobSource,
                   in_args: Option<Vec<String>>,
                   out_args: Option<Vec<String>>)
                   -> Result<s::Build001> {
        let original = CmdBuild::load_job(source)?;
        let a = CmdBuild::inject(original, in_args, s::IoDirection::In)?;
        CmdBuild::inject(a, out_args, s::IoDirection::Out)
    }
    pub fn parse(source: JobSource,
                 in_args: Option<Vec<String>>,
                 out_args: Option<Vec<String>>)
                 -> CmdBuild {
        CmdBuild {
            job: CmdBuild::parse_maybe(source, in_args, out_args),
            response: None,
        }
    }




    fn transform_build(b: s::Build001, directory: &Path) -> Result<(Vec<String>,s::Build001)>{

        let mut log = Vec::new();
        let transformed = b.io.into_iter().map(|obj| {
            let e: s::IoEnum = obj.io;
            let new_enum = if obj.direction == s::IoDirection::In{
                match e{
                    s::IoEnum::Filename(path) => {
                        let fname = format!("input_{}_{}", obj.io_id, std::path::Path::new(&path).file_name().unwrap().to_str().unwrap());
                        let new_path = directory.join(&fname).as_os_str().to_str().unwrap().to_owned();
                        std::fs::copy(&path, &new_path).unwrap();
                        log.push(format!("Copied {} to {} (referenced as {})", &path, &new_path, &fname));
                        s::IoEnum::Filename(fname)
                    }
                    s::IoEnum::Url(url) => {

                        let fname = format!("input_{}", obj.io_id);
                        let new_path = directory.join(&fname).as_os_str().to_str().unwrap().to_owned();
                        let bytes = ::imageflow_helpers::fetching::fetch_bytes(&url).unwrap();
                        let mut file = BufWriter::new(File::create(&new_path).unwrap());
                        file.write(&bytes).unwrap();
                        log.push(format!("Downloaded {} to {} (referenced as {})", &url, &new_path, &fname));
                        s::IoEnum::Filename(fname)
                    }
                    other => other
                }
            }else{
                match e{
                    s::IoEnum::Filename(path) => {
                        let fname = format!("output_{}_{}", obj.io_id, &std::path::Path::new(&path).file_name().unwrap().to_str().unwrap());
                        //let new_path = directory.join(&fname).as_os_str().to_str().unwrap().to_owned();
                        log.push(format!("Changed output {} to {}", &path, &fname));
                        s::IoEnum::Filename(fname)
                    }
                    other => other
                }
            };
            s::IoObject{
                direction: obj.direction,
                io: new_enum,
                io_id: obj.io_id
            }
        }).collect::<Vec<s::IoObject>>();
        Ok((log, s::Build001{
            io: transformed,
            builder_config: b.builder_config,
            framewise: b.framewise
        }))

    }
    fn write_json<T,P: AsRef<Path>>(path: &P, info: &T)
        where T: serde::Serialize
    {
        let mut file = BufWriter::new(File::create(path).unwrap());
        write!(file, "{}", serde_json::to_string_pretty(info).unwrap()).unwrap();
    }


    // Write new invocation to STDOUT, for execution in 'directory'.
    // Will write recipe and dependencies into directory
    pub fn bundle_to(self, directory: &Path) -> i32{
        std::fs::create_dir(directory).unwrap();
        let (log, transformed) = CmdBuild::transform_build(self.job.unwrap(), directory).unwrap();
        CmdBuild::write_json(&directory.join("recipe.json"), &transformed);
        println!("cd {:?}", &directory);
        println!("imageflow_tool --json recipe.json\n\n");
        for s in log {
            println!("# {}",&s);
        }
        0
    }

    pub fn build_maybe(self) -> CmdBuild {

        if self.job.is_ok() {

            let mut result = Err(CmdError::Incomplete);
            {
                if let Ok(ref b) = self.job {
                    result = CmdBuild::build(&b);
                }
            }
            CmdBuild { response: Some(result), ..self }
        } else {
            self
        }
    }

//    pub fn get_modified_recipe<'a>(&'a self) -> &'a Result<s::Build001>{
//        &self.job
//    }

    ///
    /// Write the JSON response (if present) to the given file or STDOUT
    pub fn write_response_maybe(&self, response_file: Option<&str>) -> std::io::Result<()> {
        if let Some(ref rr) = self.response {
            if let &Ok(ref r) = rr {
                if let Some(ref filename) = response_file {
                    let mut file = BufWriter::new(File::create(filename).unwrap());
                    file.write(&r.response_json)?;
                } else {
                    std::io::stdout().write(&r.response_json)?;
                }
            }
        }
        Ok(())
    }

    pub fn write_errors_maybe(&self) -> std::io::Result<()> {
        let err = &mut std::io::stderr();

        if let Err(ref e) = self.job {
            writeln!(err, "{:?}", e)?;
        }

        if let Some(ref rr) = self.response {
            match *rr {
                Err(ref e) => {
                    writeln!(err, "{:?}", e)?;
                }
                Ok(ref response) => {
                    let happy_json = response.status_code >= 200 && response.status_code < 300;
                    if !happy_json {
                        writeln!(err,
                                 "Job failed with status code {}. Response:\n",
                                 response.status_code)?;
                        std::io::stderr().write(&response.response_json)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_exit_code(&self) -> Option<i32> {
        if let Err(ref e) = self.job {
            return Some(e.exit_code());
        }
        if let Some(ref rr) = self.response {
            match *rr {
                Err(ref err) => Some(err.exit_code()),
                Ok(ref response) => {
                    match response.status_code {
                        s if s >= 200 && s < 300 => Some(0),
                        s if s >= 400 && s < 500 => Some(65), //user error, bad data
                        _ => Some(70), //internal error
                    }
                }
            }
        } else {
            None
        }
    }

    fn build<'a, 'b>(data: &'a s::Build001) -> Result<fc::JsonResponse> {
        let mut context = fc::Context::create()?;
        Ok(context.message("v0.1/build", &serde_json::to_vec(data).unwrap())?)
    }
}
