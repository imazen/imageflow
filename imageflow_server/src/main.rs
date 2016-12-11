extern crate iron;
extern crate router;
extern crate rustc_serialize;
extern crate hyper;
extern crate libc;
extern crate time;
extern crate clap;

extern crate imageflow_core;
extern crate imageflow_types as s;

use clap::{App, Arg, SubCommand};

use hyper::Client;
use imageflow_core::boring::*;
use imageflow_core::clients::stateless;
use imageflow_core::ffi::*;

use iron::mime::*;
use iron::prelude::*;
use iron::status;
use router::Router;
use std::io::Read;
use std::str::FromStr;
use time::precise_time_ns;



// Todo: consider lru_cache crate

#[derive(Debug)]
pub enum ServerError {
    HyperError(hyper::Error),
    IoError(std::io::Error),
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
fn fetch_bytes(url: &str) -> std::result::Result<(Vec<u8>, u64), ServerError> {
    let start = precise_time_ns();

    let client = Client::new();
    let mut res = client.get(url).send()?;

    if res.status != hyper::Ok {
        return Err(ServerError::UpstreamResponseError(res.status));
    }

    let mut source_bytes = Vec::new();
    let _ = res.read_to_end(&mut source_bytes)?;

    let downloaded = precise_time_ns();
    Ok((source_bytes, downloaded - start))
}

fn error_upstream(from: ServerError) -> ServerError {
    match from {
        ServerError::HyperError(e) => ServerError::UpstreamHyperError(e),
        ServerError::IoError(e) => ServerError::UpstreamIoError(e),
        e => e,
    }
}

struct RequestPerf {
    fetch_ns: u64,
    get_image_info_ns: u64,
    execute_ns: u64,
}
impl RequestPerf {
    fn debug(&self) -> String {
        format!("HTTP fetch took: {} ms, get_image_info took {} ms, execute took {} ms",
                (self.fetch_ns as f64) / 1000000.0,
                (self.get_image_info_ns as f64) / 1000000.0,
                (self.execute_ns as f64) / 1000000.0)
    }
}

fn execute_one_to_one<F>
    (source: &str,
     framewise_generator: F)
     -> std::result::Result<(stateless::BuildOutput, RequestPerf), ServerError>
    where F: Fn(s::ImageInfo) -> s::Framewise
{
    let (original_bytes, fetch_ns) = fetch_bytes(source).map_err(error_upstream)?;
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
        fetch_ns: fetch_ns,
        get_image_info_ns: start_execute - start_get_info,
        execute_ns: end_execute - start_execute,
    }))
}

fn respond_one_to_one<F>(source: &str, framewise_generator: F) -> IronResult<Response>
    where F: Fn(s::ImageInfo) -> s::Framewise
{
    match execute_one_to_one(source, framewise_generator) {
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
    let router = req.extensions.get::<Router>().unwrap();
    let w = router.find("w").and_then(|x| x.parse::<u32>().ok());
    let h = router.find("h").and_then(|x| x.parse::<u32>().ok());
    let url = "http://images.unsplash.com/".to_string() + router.find("url").unwrap();

    respond_one_to_one(&url, |info: s::ImageInfo| {
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

// use serde_toml
// Deserialize from TOML or from inline struct

// pub struct Hostname
//
// pub struct PerRequestLimits{
//    max_pixels_out: Option<i64>,
//    max_pixels_in: Option<i64>,
//    max_cpu_milliseconds: Option<i64>,
//    max_bitmap_ram_bytes: Option<i64>
// }
//
// pub struct ContentTypeRestrictions{
//    allow: Option<Vec<Mime>>,
//    deny: Option<Vec<Mime>>,
//    allow_extensions: Option<Vec<String>>,
//    deny_extensions: Option<Vec<String>>
// }
// pub struct SecurityPolicy{
//    per_request_limits: Option<PerRequestLimits>,
//    serve_content_types: Option<ContentTypeRestrictions>,
//    proxy_content_types: Option<ContentTypeRestrictions>,
//    force_image_recoding: Option<bool>
// }
//
// pub enum BlobSource{
//    Directory(String),
//    HttpServer(String),
//    //TODO: Azure and S3 blob backens
// }
//
// pub enum InternalCachingStrategy{
//    PubSubAndPermaPyramid,
//    TrackStatsAndPermaPyramid,
//    OpportunistPermaPyramid,
//    PubSubToInvalidate,
//    OpportunistPubSubEtagCheck,
//
// }
// pub struct CacheControlPolicy{
// //How do we set etag/last modified/expires/maxage?
// }
//
// pub struct BaseConfig{
//    //Security defaults
//    pub security: Option<SecurityPolicy>,
//    //May also want to filter by hostnames or ports for heavy multi-tenanting
//    pub cache_control: Option<CacheControlPolicy>
// }
//
// pub enum Frontend{
//    ImageResizer4Compatible,
//    Flow0
// }
// pub struct MountPath {
//    //Where we get originals from
//    pub source: BlobSource,
//    //The virtual path for which we handle sub-requests.
//    pub prefix: String,
//    //Customize security
//    pub security: Option<SecurityPolicy>,
//    //May also want to filter by hostnames or ports for heavy multi-tenanting
//    pub cache_control: Option<CacheControlPolicy>,
//
//    pub api: Frontend
// }

fn main() {
    let exit_code = main_with_exit_code();
    std::process::exit(exit_code);
}

fn serve(){
    let mut router = Router::new();

    // Mount prefix (external) (url|relative path)
    // pass through static files (whitelisted??)


    router.get("/proto1/scale_unsplash_jpeg/:w/:h/:url",
               move |r: &mut Request| proto1(r),
               "proto1-unsplash");

    router.get("/",
               move |r: &mut Request|
                   Ok(Response::with((iron::status::Ok, "Hello World"))), "home-hello");

    Iron::new(router).http("0.0.0.0:3000").unwrap();
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
        serve();
        return 0;
    }
    64
}

