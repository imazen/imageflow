extern crate iron;
extern crate persistent;
extern crate router;
extern crate rustc_serialize;
extern crate hyper;
extern crate libc;
extern crate time;
extern crate clap;
extern crate imageflow_server;
extern crate imageflow_helpers;
extern crate imageflow_core;
extern crate imageflow_types as s;
use ::imageflow_helpers as hlp;
use ::imageflow_helpers::preludes::from_std::*;

use clap::{App, Arg, SubCommand};

use hyper::Client;

use imageflow_core::boring::*;
use imageflow_core::clients::stateless;
use imageflow_core::ffi::*;
use ::imageflow_server::disk_cache::{CacheFolder, CacheEntry, FolderLayout};



use iron::mime::*;
use iron::prelude::*;
use iron::status;
use router::Router;
use iron::typemap::Key;

use time::precise_time_ns;


#[derive(Debug)]
struct SharedData{
    source_cache: CacheFolder
}

impl iron::typemap::Key for SharedData { type Value = SharedData; }


// Todo: consider lru_cache crate

#[derive(Debug)]
pub enum ServerError {
    HyperError(hyper::Error),
    IoError(std::io::Error),
    DiskCacheReadIoError(std::io::Error),
    DiskCacheWriteIoError(std::io::Error),
    UpstreamResponseError(hyper::status::StatusCode),
    UpstreamHyperError(hyper::Error),
    UpstreamIoError(std::io::Error),
    BuildFailure(stateless::BuildFailure),
}
impl From<stateless::BuildFailure> for ServerError {
    fn from(e: stateless::BuildFailure) -> ServerError {
        ServerError::BuildFailure(e)
    }
}
impl From<hyper::Error> for ServerError {
    fn from(e: hyper::Error) -> ServerError {
        ServerError::HyperError(e)
    }
}
impl From<std::io::Error> for ServerError {
    fn from(e: std::io::Error) -> ServerError {
        ServerError::IoError(e)
    }
}
fn fetch_bytes(url: &str) -> std::result::Result<(Vec<u8>, AcquirePerf), ServerError> {
    let start = precise_time_ns();
    let client = Client::new();
    let mut res = client.get(url).send()?;
    if res.status != hyper::Ok {
        return Err(ServerError::UpstreamResponseError(res.status));
    }
    let mut source_bytes = Vec::new();

    use std::io::Read;

    let _ = res.read_to_end(&mut source_bytes)?;
    let downloaded = precise_time_ns();
    Ok((source_bytes, AcquirePerf{ fetch_ns: downloaded - start, .. Default::default()}))
}

fn error_upstream(from: ServerError) -> ServerError {
    match from {
        ServerError::HyperError(e) => ServerError::UpstreamHyperError(e),
        ServerError::IoError(e) => ServerError::UpstreamIoError(e),
        e => e,
    }
}

fn error_cache_read(from: ServerError) -> ServerError {
    match from {
        ServerError::IoError(e) => ServerError::UpstreamIoError(e),
        e => e,
    }
}



// Additional ways this can fail (compared to fetch_bytes)
// Parent directories are deleted from cache between .exists() and cache writes
// Permissions issues
// Cached file is deleted between .exists(0 and .read()
// Write fails due to out-of-space
// rename fails (it should overwrite, for eventual consistency, but ... filesystems)
fn fetch_bytes_using_cache_by_url(cache: &CacheFolder, url: &str) -> std::result::Result<(Vec<u8>, AcquirePerf), ServerError>{
    let hash = hlp::hashing::hash_256(url.as_bytes());
    let entry = cache.entry(&hash);
    if entry.exists(){
        let start = precise_time_ns();
        match entry.read() {
            Ok(vec) => {
                let end = precise_time_ns();
                Ok((vec, AcquirePerf { cache_read_ns: end - start, ..Default::default() }))
            },
            Err(e) => Err(ServerError::DiskCacheReadIoError(e))
        }
    }else{
        let result = fetch_bytes(url);
        if let Ok((bytes, perf)) = result {
            let start = precise_time_ns();
            match entry.write(&bytes) {
                Ok(()) => {
                    let end = precise_time_ns();
                    Ok((bytes, AcquirePerf { cache_write_ns: end - start, ..perf }))
                },
                Err(e) => Err(ServerError::DiskCacheWriteIoError(e))
            }
        }else{
            result.map_err(error_upstream)
        }
    }
}

#[derive(Default, Copy, Clone, Debug)]
struct AcquirePerf{
    fetch_ns: u64,
    cache_read_ns: u64,
    cache_write_ns: u64
}
impl AcquirePerf{
    pub fn new() -> AcquirePerf{
        AcquirePerf{fetch_ns: 0, cache_write_ns: 0, cache_read_ns: 0}
    }
    fn debug(&self) -> String {
        format!("HTTP fetch took: {} ms, cache read {} ms, cache write {} ms",
                (self.fetch_ns as f64) / 1000000.0,
                (self.cache_read_ns as f64) / 1000000.0,
                (self.cache_write_ns as f64) / 1000000.0)
    }
}

struct RequestPerf {
    acquire: AcquirePerf,
    get_image_info_ns: u64,
    execute_ns: u64,
}
impl RequestPerf {
    fn debug(&self) -> String {
        format!("{}, get_image_info took {} ms, execute took {} ms",
                self.acquire.debug(),
                (self.get_image_info_ns as f64) / 1000000.0,
                (self.execute_ns as f64) / 1000000.0)
    }
}




fn execute_one_to_one<F>
    (shared: &SharedData, source: &str,
     framewise_generator: F)
     -> std::result::Result<(stateless::BuildOutput, RequestPerf), ServerError>
    where F: Fn(s::ImageInfo) -> s::Framewise
{
    let (original_bytes, acquire_perf) = fetch_bytes_using_cache_by_url(&shared.source_cache, source).map_err(error_upstream)?;
    let mut client = stateless::LibClient {};
    let start_get_info = precise_time_ns();
    let info = client.get_image_info(&original_bytes)?;
    let start_execute = precise_time_ns();

    let result: stateless::BuildSuccess = client.build(stateless::BuildRequest {
            framewise: framewise_generator(info),
            inputs: vec![stateless::BuildInput {
                             io_id: 0,
                             bytes: &original_bytes,
                         }],
            export_graphs_to: None,
        })?;
    let end_execute = precise_time_ns();
    Ok((result.outputs.into_iter().next().unwrap(),
        RequestPerf {
        acquire: acquire_perf,
        get_image_info_ns: start_execute - start_get_info,
        execute_ns: end_execute - start_execute,
    }))
}

fn respond_one_to_one<F>(shared: &SharedData, source: &str, framewise_generator: F) -> IronResult<Response>
    where F: Fn(s::ImageInfo) -> s::Framewise
{
    match execute_one_to_one(shared, source, framewise_generator) {
        Ok((output, perf)) => {
            let mime = output.mime_type
                .parse::<Mime>()
                .unwrap_or(Mime::from_str("application/octet-stream").unwrap());

            Ok(Response::with((mime, status::Ok, output.bytes)))
        }
        Err(e) => {
            let bytes = format!("{:?}", e).into_bytes();
            Ok(Response::with((Mime::from_str("text/plain").unwrap(),
                               status::InternalServerError,
                               bytes)))
            // TODO: get a bit more specific with the error codes
        }
    }
}

fn proto1(req: &mut Request) -> IronResult<Response> {

    let w;
    let h;
    let url;
    {
        let router = req.extensions.get::<Router>().unwrap();

        let generic_url = req.url.clone().into_generic_url();

        let router_w = router.find("w").and_then(|v| Some(v.to_owned()));

        w = generic_url.query_pairs().find(|&(ref k,ref v)| k == "w").map(|(k,v)| v.into_owned()).or(router_w).and_then(|x| x.parse::<u32>().ok());

        h = router.find("h").and_then(|x| x.parse::<u32>().ok());
        url = "http://images.unsplash.com/".to_string() + router.find("url").unwrap();
    }
    let shared = req.get::<persistent::Read<SharedData>>().unwrap();

    respond_one_to_one(shared.as_ref(), &url, |info: s::ImageInfo| {
        let commands = BoringCommands {
            fit: ConstraintMode::Max,
            w: w.and_then(|w| Some(w as i32)),
            h: h.and_then(|h| Some(h as i32)),
            jpeg_quality: 90,
            precise_scaling_ratio: 2.1f32,
            luma_correct: true,
            sharpen: 0f32,
            format: ImageFormat::Jpeg,
            down_filter: Filter::Robidoux,
            up_filter: Filter::Ginseng,
        };
        let (framewise, (pre_w, pre_h)) =
            create_framewise(info.image_width, info.image_height, commands).unwrap();
        framewise
    })
}


fn main() {
    let exit_code = main_with_exit_code();
    std::process::exit(exit_code);
}

fn serve(source_cache: &Path, source_cache_layout: FolderLayout) {
    let shared_data = SharedData {
        source_cache: CacheFolder::new(source_cache, source_cache_layout),
    };
    let mut router = Router::new();

    // Mount prefix (external) (url|relative path)
    // pass through static files (whitelisted??)


    router.get("/proto1/scale_unsplash_jpeg/:w/:h/:url",
               move |r: &mut Request| { proto1(r) },
               "proto1-unsplash");

    router.get("/",
               move |r: &mut Request| {
                   Ok(Response::with((iron::status::Ok, "Hello World")))
               }
               , "home-hello");

    let mut chain = Chain::new(router);
    chain.link(persistent::Read::<SharedData>::both(shared_data));

    Iron::new(chain).http("0.0.0.0:3000").unwrap();
}

fn main_with_exit_code() -> i32 {
    let version = s::version::one_line_version();
    let app = App::new("imageflow_server").version(version.as_ref())
        .subcommand(
            SubCommand::with_name("diagnose")
                .about("Diagnostic utilities")
                .arg(
                    Arg::with_name("show-compilation-info").long("show-compilation-info")
                        .help("Show all the information stored in this executable about the environment in which it was compiled.")
                ).arg(
                Arg::with_name("call-panic").long("call-panic")
                    .help("Triggers a Rust panic (so you can observe failure/backtrace behavior)")
            )
        )
        .subcommand(
            SubCommand::with_name("start")
                .about("Start server")
            //                    .arg(
            //                        Arg::with_name("generate").long("generate")
            //                            .help("Create an 'examples' directory")
            //                    )
        );

    // --json [path]
    // --response [response_json_path]
    // --demo [name]
    // --in 0 a.png b.png
    // --out a.png

    //Eventually:
    // --local-only (prevent remote URL requests)
    // --no-io-ids (Disables interpretation of numbers in --in and --out as io_id assignment).
    // --no-clobber
    // --debug (verbose, graph export, frame export?)
    // --debug-package


    // file.json --in a.png a.png --out s.png
    // file.json --in 0 a.png 1 b.png --out 3 base64


    let matches = app.get_matches();

    if let Some(ref matches) = matches.subcommand_matches("diagnose") {
        let m: &&clap::ArgMatches = matches;

        if m.is_present("show-compilation-info") {
            println!("{}\n{}\n",
                     s::version::one_line_version(),
                     s::version::all_build_info_pairs());
            return 0;
        }
        if m.is_present("call-panic") {
            panic!("Panicking on command");
        }
    }
    if let Some(ref matches) = matches.subcommand_matches("start") {

        let source_cache_dir = Path::new("source_cache");
        serve(&source_cache_dir, FolderLayout::Tiny );
        return 0;
    }
    64
}

