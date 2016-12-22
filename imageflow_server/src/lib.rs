#![feature(integer_atomics)]
#![feature(conservative_impl_trait)]

extern crate iron;
extern crate persistent;
extern crate router;
extern crate logger;

extern crate rustc_serialize;
extern crate hyper;
extern crate libc;
extern crate time;
#[macro_use] extern crate lazy_static;
extern crate regex;

use std::sync::atomic::{AtomicU64, AtomicBool, ATOMIC_U64_INIT};
use std::sync::atomic;

use regex::Regex;

extern crate imageflow_helpers;
extern crate imageflow_core;
extern crate imageflow_types as s;
extern crate imageflow_riapi;

use ::imageflow_helpers as hlp;
use ::imageflow_helpers::preludes::from_std::*;
use imageflow_core::clients::stateless;

use hyper::Url;
use hyper::Client;

pub mod disk_cache;
pub mod resizer;

use disk_cache::{CacheFolder, CacheEntry, FolderLayout};
use logger::Logger;

pub mod preludes{
    pub use super::{MountedEngine, MountLocation, StartServerConfig, ServerError};
    pub use super::disk_cache::FolderLayout;
}


use iron::{AfterMiddleware, BeforeMiddleware};
use iron::mime::*;
use iron::prelude::*;
use iron::status;
use router::Router;
use iron::typemap::Key;

use time::precise_time_ns;

#[macro_use] extern crate log;
extern crate env_logger;

use log::LogLevel;

#[derive(Debug)]
struct SharedData{
    source_cache: CacheFolder,
    output_cache: CacheFolder,
    requests_received: AtomicU64
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
    LayoutSizingError(::imageflow_riapi::sizing::LayoutError)
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

fn fetch_bytes_from_disk(url: &Path) -> std::result::Result<(Vec<u8>, AcquirePerf), ServerError> {
    let start = precise_time_ns();
    let vec = hlp::filesystem::read_file_bytes(url)?;
    let end = precise_time_ns();
    Ok((vec, AcquirePerf { cache_read_ns: end - start, ..Default::default() }))
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



fn execute_using<F, F2>(bytes_provider: F2, framewise_generator: F)
 -> std::result::Result<(stateless::BuildOutput, RequestPerf), ServerError>
    where F: Fn(s::ImageInfo) -> std::result::Result<s::Framewise, ServerError>,
          F2: Fn() -> std::result::Result<(Vec<u8>, AcquirePerf), ServerError>,
{
    let (original_bytes, acquire_perf) = bytes_provider()?;
    let mut client = stateless::LibClient {};
    let start_get_info = precise_time_ns();
    let info = client.get_image_info(&original_bytes)?;
    let start_execute = precise_time_ns();

    let result: stateless::BuildSuccess = client.build(stateless::BuildRequest {
        framewise: framewise_generator(info)?,
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

fn respond_using<F, F2>(bytes_provider: F2, framewise_generator: F)
                             -> IronResult<Response>
    where F: Fn(s::ImageInfo) -> std::result::Result<s::Framewise, ServerError>,
          F2: Fn() -> std::result::Result<(Vec<u8>, AcquirePerf), ServerError>,
{
    match execute_using(bytes_provider, framewise_generator){
        Ok((output, perf)) => {
            let mime = output.mime_type
                .parse::<Mime>()
                .unwrap_or(Mime::from_str("application/octet-stream").unwrap());

            Ok(Response::with((mime, status::Ok, output.bytes)))
        }
        Err(ServerError::UpstreamResponseError(hyper::status::StatusCode::NotFound)) => {
            let bytes = format!("Remote file not found (upstream server responded with 404)").into_bytes();
            Ok(Response::with((Mime::from_str("text/plain").unwrap(),
                               status::NotFound,
                               bytes)))
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
//
//fn proto1(req: &mut Request) -> IronResult<Response> {
//
//    let w;
//    let h;
//    let url;
//    {
//        let router = req.extensions.get::<Router>().unwrap();
//
//        let generic_url = req.url.clone().into_generic_url();
//
//        let router_w = router.find("w").and_then(|v| Some(v.to_owned()));
//
//        w = generic_url.query_pairs().find(|&(ref k,ref v)| k == "w").map(|(k,v)| v.into_owned()).or(router_w).and_then(|x| x.parse::<u32>().ok());
//
//        h = router.find("h").and_then(|x| x.parse::<u32>().ok());
//        url = "http://images.unsplash.com/".to_string() + router.find("url").unwrap();
//    }
//    let shared = req.get::<persistent::Read<SharedData>>().unwrap();
//
//    respond_one_to_one(shared.as_ref(), &url, |info: s::ImageInfo| {
//        let commands = BoringCommands {
//            fit: ConstraintMode::Max,
//            w: w.and_then(|w| Some(w as i32)),
//            h: h.and_then(|h| Some(h as i32)),
//            jpeg_quality: 90,
//            precise_scaling_ratio: 2.1f32,
//            luma_correct: true,
//            sharpen: 0f32,
//            format: ImageFormat::Jpeg,
//            down_filter: Filter::Robidoux,
//            up_filter: Filter::Ginseng,
//        };
//        let (framewise, (pre_w, pre_h)) =
//        create_framewise(info.image_width, info.image_height, commands).unwrap();
//        framewise
//    })
//}

fn ir4_http_respond<F>(shared: &SharedData, url: &str, framewise_generator: F) -> IronResult<Response>
    where F: Fn(s::ImageInfo) -> std::result::Result<s::Framewise, ServerError>
{
    respond_using(|| fetch_bytes_using_cache_by_url(&shared.source_cache, url).map_err(error_upstream), framewise_generator)
}



fn ir4_framewise(info: s::ImageInfo, url: &Url) -> std::result::Result<s::Framewise, ServerError>{
    ::imageflow_riapi::ir4::parse_to_framewise(info, url).map_err(|e| ServerError::LayoutSizingError(e)).map(|(framewise, warnings)| framewise)
}


type EngineHandler<T> = fn(req: &mut Request, engine_data: &T, mount: &MountLocation) -> IronResult<Response> ;
type EngineSetup<T> = fn(mount: &MountLocation, router: &mut Router) ->  Result<(T,EngineHandler<T>),String>  ;



fn ir4_local_respond<F>(shared: &SharedData, source: &Path, framewise_generator: F) -> IronResult<Response>
    where F: Fn(s::ImageInfo) -> std::result::Result<s::Framewise, ServerError>
{
    respond_using(|| fetch_bytes_from_disk(source), framewise_generator)
}
fn ir4_local_handler(req: &mut Request, local_path: &PathBuf, mount: &MountLocation) -> IronResult<Response>{
    let path = req.extensions.get::<Router>().unwrap().find("path").unwrap().to_owned();
    let url = req.url.clone().into_generic_url();
    let shared = req.get::<persistent::Read<SharedData>>().unwrap();
    //Ensure the combined path is canonical.
    let original = local_path.join(path);
    if let Ok(canonical) = original.canonicalize(){
        if canonical.exists() && original == canonical{
            return ir4_local_respond(&shared, canonical.as_path(), move | info: s::ImageInfo| {
                ir4_framewise(info, &url)
            });
        }
    }
    let bytes = format!("File not found").into_bytes();
    Ok(Response::with((Mime::from_str("text/plain").unwrap(),
                       status::NotFound,
                       bytes)))

}
fn ir4_local_setup(mount: &MountLocation, router: &mut Router) ->  Result<(PathBuf,EngineHandler<PathBuf>),String>{
    if mount.engine_args.len() < 1{
        Err("ir4_local requires at least one argument - the path to the physical folder it is serving".to_owned())
    }else {
        //TODO: validate path
        let local_dir = Path::new(&mount.engine_args[0]).canonicalize().map_err(|e| format!("{:?}",e))?;
        Ok((local_dir, ir4_local_handler))
    }
}



fn ir4_http_handler(req: &mut Request, base_url: &String, mount: &MountLocation) -> IronResult<Response> {
    let path = req.extensions.get::<Router>().unwrap().find("path").unwrap().to_owned();
    let url = req.url.clone().into_generic_url();
    let shared = req.get::<persistent::Read<SharedData>>().unwrap();
    //TODO: Ensure the combined url is canonical (or, at least, lacks ..)
    let remote_url = format!("{}{}", base_url, path);

    ir4_http_respond(&shared, &remote_url, move |info: s::ImageInfo| {
        ir4_framewise(info, &url)
    })
}

fn ir4_http_setup(mount: &MountLocation, router: &mut Router) -> Result<(String,EngineHandler<String>),String> {
    if mount.engine_args.len() < 1 {
        Err("ir4_http requires at least one argument - the base url to suffix paths to".to_owned())
    } else {
        Ok((mount.engine_args[0].to_owned(), ir4_http_handler))
    }
}



fn mount<T>(method: iron::method::Method, mount: MountLocation, router: &mut Router, setup: EngineSetup<T>) -> Result<(),String>
    where T: Send, T: Sync, T: 'static {
    let (data, handler) = setup(&mount, router)?;

    let glob = format!("{}:path", &mount.prefix);
    let route_id = format!("{}_{:?}", &mount.prefix.replace("/", "_"), method);
    router.route(method, &glob, move |r: &mut Request| { handler(r, &data, &mount) }, &route_id);
    Ok(())
}

pub fn serve(c: StartServerConfig) {
    env_logger::init().unwrap();

    let shared_data = SharedData {
        source_cache: CacheFolder::new(c.data_dir.join(Path::new("source_cache")).as_path(), c.default_cache_layout.unwrap_or(FolderLayout::Normal)),
        output_cache: CacheFolder::new(c.data_dir.join(Path::new("output_cache")).as_path(), c.default_cache_layout.unwrap_or(FolderLayout::Normal)),
        requests_received: ATOMIC_U64_INIT //NOT YET USED
    };
    let mut router = Router::new();

    // Mount prefix (external) (url|relative path)
    // pass through static files (whitelisted??)

    for m in c.mounts.into_iter(){
        let mount_result = match m.engine {
            //MountedEngine::Ir4Https => "ir4_https",
            MountedEngine::Ir4Http => mount(iron::method::Method::Get, m, &mut router, ir4_http_setup),
            MountedEngine::Ir4Local => mount(iron::method::Method::Get, m, &mut router, ir4_local_setup),
        };
        mount_result.unwrap();
//        if let Err(s) = mount_result{
//            panic!("Failed to mount {} using engine {}: {}", m.prefix.as_str(), m.engine.to_id(), &s);
//        }
    }

    if c.integration_test {
        router.get("/test/shutdown", move |r: &mut Request| -> IronResult<Response> {
            println!("Stopping server due to GET /test/shutdown");
            std::process::exit(0);

//            Ok(Response::with((Mime::from_str("text/plain").unwrap(),
//                               status::InternalServerError,
//                               bytes)))
        }, "test-shutdown");
    }

    let mut chain = Chain::new(router);

    chain.link(persistent::Read::<SharedData>::both(shared_data));

    let (logger_before, logger_after) = Logger::new(None);

    // Link logger_before as your first before middleware.
    chain.link_before(logger_before);

    // Link logger_after as your *last* after middleware.
    chain.link_after(logger_after);

    println!("Listening on {}", c.bind_addr.as_str());
    Iron::new(chain).http(c.bind_addr.as_str()).unwrap();
}



#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MountedEngine{
    Ir4Local,
    Ir4Http,
    //Ir4Https
}

impl MountedEngine{
    pub fn to_id(&self) -> &'static str{
        match *self{
            //MountedEngine::Ir4Https => "ir4_https",
            MountedEngine::Ir4Http => "ir4_http",
            MountedEngine::Ir4Local=> "ir4_local",
        }
    }
    pub fn from_id(s: &str) -> Option<MountedEngine> {
        match s {
            "ir4_local" => Some(MountedEngine::Ir4Local),
            "ir4_http" => Some(MountedEngine::Ir4Http),
            //"ir4_https" => Some(MountedEngine::Ir4Https),
            _ => None
        }
    }

    pub fn id_values() -> &'static [&'static str] {
        static ID_VALUES: [&'static str;2] = ["ir4_local", "ir4_http",/* "ir4_https"*/];

        &ID_VALUES
    }
}

trait Engine{
    fn mount(self, mount: MountLocation, router: &mut Router) -> Result<(),String>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct MountLocation{
 pub prefix: String,
 pub engine: MountedEngine,
 pub engine_args: Vec<String>,
 //TODO: HTTPS
}

impl MountLocation{
    pub fn parse(prefix: String, engine_name: String, args: Vec<String>) -> std::result::Result<MountLocation, String>{
        lazy_static!{
            static ref RE: Regex = Regex::new(r"\A(/[a-zA-Z0-9-]+?)+?/\z").unwrap();
        }
        if !RE.is_match(&prefix){
            return Err("mount points must be valid paths with leading and trailing slashes, like /img/logos/. Between slashes, [a-zA-Z0-9-] may be used".to_owned());
        }
        let engine = MountedEngine::from_id(engine_name.as_str());

        if engine.is_none(){
            return Err(format!("Valid engine names include {:?}. Provided {}", MountedEngine::id_values(), engine_name.as_str()));
        }

        Ok(MountLocation{
            prefix: prefix,
            engine: engine.unwrap(),
            engine_args: args
        })
    }
}


#[derive(Debug, Clone, PartialEq)]
pub struct StartServerConfig{
    pub data_dir: PathBuf,
    pub bind_addr: String,
    pub mounts: Vec<MountLocation>,
    pub default_cache_layout: Option<FolderLayout>,
    pub integration_test: bool
}
