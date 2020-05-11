
extern crate iron;
extern crate persistent;
extern crate router;
extern crate logger;

extern crate bincode;
extern crate mount;

use staticfile::Static;


#[macro_use] extern crate serde_derive;

extern crate staticfile;
extern crate rustc_serialize;
extern crate hyper;

extern crate time;
#[macro_use] extern crate lazy_static;
extern crate regex;

extern crate hyper_native_tls;

use hyper_native_tls::NativeTlsServer;

use std::sync::atomic::AtomicUsize;


extern crate conduit_mime_types as mime_types;

use regex::Regex;

extern crate imageflow_helpers;
extern crate imageflow_core;
extern crate imageflow_types as s;
extern crate imageflow_riapi;
extern crate reqwest;

use ::imageflow_helpers as hlp;
use imageflow_http_helpers::FetchConfig;
use imageflow_helpers::preludes::from_std::*;
use imageflow_core::clients::stateless;


pub mod disk_cache;
pub mod resizer;
pub mod diagnose;

mod requested_path;
extern crate url;

use crate::disk_cache::{CacheFolder,  FolderLayout};
use logger::Logger;

pub mod preludes {
    pub use super::{MountedEngine, MountLocation, StartServerConfig, ServerError};
    pub use super::disk_cache::FolderLayout;
}


use iron::mime::*;
use iron::prelude::*;
use iron::status;
use router::Router;





use imageflow_helpers::timeywimey::precise_time_ns;

#[cfg_attr(feature = "cargo-clippy", allow(useless_attribute))]
#[allow(unused_imports)]
#[macro_use] extern crate log;
extern crate env_logger;


#[derive(Debug)]
struct SharedData {
    source_cache: CacheFolder,
    output_cache: CacheFolder,
    requests_received: AtomicUsize,
    //detailed_errors: bool
}

impl iron::typemap::Key for SharedData { type Value = SharedData; }


// Todo: consider lru_cache crate

#[derive(Debug)]
pub enum ServerError {
    HyperError(hyper::Error),
    ReqwestError(reqwest::Error),
    IoError(std::io::Error),
    DiskCacheReadIoError(std::io::Error),
    DiskCacheWriteIoError(std::io::Error),
    UpstreamResponseError(reqwest::StatusCode),
    UpstreamHyperError(hyper::Error),
    UpstreamReqwestError(reqwest::Error),
    UpstreamIoError(std::io::Error),
    BuildFailure(stateless::BuildFailure),
    LayoutSizingError(::imageflow_riapi::sizing::LayoutError)
}

impl From<stateless::BuildFailure> for ServerError {
    fn from(e: stateless::BuildFailure) -> ServerError {
        ServerError::BuildFailure(e)
    }
}

impl From<reqwest::Error> for ServerError {
    fn from(e: reqwest::Error) -> ServerError {
        ServerError::ReqwestError(e)
    }
}
impl From<hyper::Error> for ServerError {
    fn from(e: hyper::Error) -> ServerError {
        ServerError::HyperError(e)
    }
}
impl From<::imageflow_http_helpers::FetchError> for ServerError {
    fn from(e: ::imageflow_http_helpers::FetchError) -> ServerError {
        match e{
            ::imageflow_http_helpers::FetchError::HyperError(e) => ServerError::HyperError(e),
            ::imageflow_http_helpers::FetchError::IoError(e) => ServerError::IoError(e),
            ::imageflow_http_helpers::FetchError::UpstreamResponseError(e) => ServerError::UpstreamResponseError(e),
            ::imageflow_http_helpers::FetchError::UpstreamResponseErrorWithResponse{status, ..}=> ServerError::UpstreamResponseError(status),
            ::imageflow_http_helpers::FetchError::ReqwestError(e) => ServerError::ReqwestError(e)
        }

    }
}

impl From<std::io::Error> for ServerError {
    fn from(e: std::io::Error) -> ServerError {
        ServerError::IoError(e)
    }
}

struct FetchedResponse {
    bytes: Vec<u8>,
    perf: AcquirePerf,
    content_type: reqwest::header::HeaderValue,
}

fn fetch_bytes(url: &str, config: Option<FetchConfig>) -> std::result::Result<FetchedResponse, ServerError> {
    let start = precise_time_ns();
    let result = ::imageflow_http_helpers::fetch(url, config);
    let downloaded = precise_time_ns();

    match result{
        Ok(r) => Ok(FetchedResponse{
            bytes: r.bytes,
            content_type: r.content_type,
            perf: AcquirePerf { fetch_ns: downloaded - start, ..Default::default() }
        }),
        Err(e) => Err(error_upstream(e.into()))
    }
}

fn error_upstream(from: ServerError) -> ServerError {
    match from {
        ServerError::HyperError(e) => ServerError::UpstreamHyperError(e),
        ServerError::ReqwestError(e) => ServerError::UpstreamReqwestError(e),
        ServerError::IoError(e) => ServerError::UpstreamIoError(e),
        e => e,
    }
}




#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
struct CachedResponse {
    bytes: Vec<u8>,
    content_type: String,
}

// Additional ways this can fail (compared to fetch_bytes)
// Parent directories are deleted from cache between .exists() and cache writes
// Permissions issues
// Cached file is deleted between .exists(0 and .read()
// Write fails due to out-of-space
// rename fails (it should overwrite, for eventual consistency, but ... filesystems)
fn fetch_bytes_using_cache_by_url(cache: &CacheFolder, url: &str) -> std::result::Result<(Vec<u8>, AcquirePerf), ServerError> {
    let hash = hlp::hashing::hash_256(url.as_bytes());
    let entry = cache.entry(&hash);
    if entry.exists() {
        let start = precise_time_ns();
        match entry.read() {
            Ok(vec) => {
                let end = precise_time_ns();
                Ok((vec, AcquirePerf { cache_read_ns: end - start, ..Default::default() }))
            },
            Err(e) => Err(ServerError::DiskCacheReadIoError(e))
        }
    } else {
        let result = fetch_bytes(url, None);
        if let Ok(FetchedResponse { bytes, perf, .. }) = result {
            let start = precise_time_ns();
            match entry.write(&bytes) {
                Ok(()) => {
                    let end = precise_time_ns();
                    Ok((bytes, AcquirePerf { cache_write_ns: end - start, ..perf }))
                },
                Err(e) => Err(ServerError::DiskCacheWriteIoError(e))
            }
        } else {
            Err(result.map_err(error_upstream).err().unwrap())
        }
    }
}

fn fetch_response_using_cache_by_url(cache: &CacheFolder, url: &str) -> std::result::Result<(CachedResponse, AcquirePerf), ServerError> {
    let hash = hlp::hashing::hash_256(url.as_bytes()); //TODO: version this
    let entry = cache.entry(&hash);
    if entry.exists() {
        let start = precise_time_ns();
        match entry.read() {
            Ok(vec) => {
                let end = precise_time_ns();
                let cached: CachedResponse =  bincode::deserialize(&vec).unwrap();
                Ok((cached, AcquirePerf { cache_read_ns: end - start, ..Default::default() }))
            },
            Err(e) => Err(ServerError::DiskCacheReadIoError(e))
        }
    } else {
        let result = fetch_bytes(url, None);
        if let Ok(fetched) = result {
            let start = precise_time_ns();
            let bytes = bincode::serialize(&fetched.bytes).unwrap();
            match entry.write(&bytes) {
                Ok(()) => {
                    let end = precise_time_ns();
                    Ok((CachedResponse { bytes: fetched.bytes, content_type: format!("{}", fetched.content_type.to_str().unwrap()) }, AcquirePerf { cache_write_ns: end - start, ..fetched.perf }))
                },
                Err(e) => Err(ServerError::DiskCacheWriteIoError(e))
            }
        } else {
            Err(result.map_err(error_upstream).err().unwrap())
        }
    }
}

fn fetch_bytes_from_disk(url: &Path) -> std::result::Result<(Vec<u8>, AcquirePerf), ServerError> {
    let start = precise_time_ns();
    let vec = hlp::filesystem::read_file_bytes(url)?;
    let end = precise_time_ns();
    Ok((vec, AcquirePerf { cache_read_ns: end - start, ..Default::default() }))
}

#[derive(Default, Copy,  Clone, Debug)]
struct AcquirePerf {
    fetch_ns: u64,
    cache_read_ns: u64,
    cache_write_ns: u64
}


impl AcquirePerf {
    fn total(&self) -> u64{
        self.fetch_ns + self.cache_read_ns + self.cache_write_ns
    }
}

struct RequestPerf {
    acquire: AcquirePerf,
    get_image_info_ns: u64,
    execute_ns: u64,
}

impl RequestPerf {
    fn short(&self) -> String {
        format!("execute {:.2}ms getinfo {:.2}ms fetch-through: {:.2}ms",
                self.execute_ns as f64 / 1_000_000.0f64
                , self.get_image_info_ns as f64 / 1_000_000.0f64,
                (self.acquire.total() as f64) / 1_000_000.0f64)
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

fn respond_using<F, F2, A>(debug_info: &A, bytes_provider: F2, framewise_generator: F)
                        -> IronResult<Response>
    where F: Fn(s::ImageInfo) -> std::result::Result<s::Framewise, ServerError>,
          F2: Fn() -> std::result::Result<(Vec<u8>, AcquirePerf), ServerError>,
    A: std::fmt::Debug
{
    //TODO: support process=, cache=, etc? pass-through by default?
    match execute_using(bytes_provider, framewise_generator) {
        Ok((output, perf)) => {
            let mime = output.mime_type
                .parse::<Mime>()
                .unwrap_or_else(|_| Mime::from_str("application/octet-stream").unwrap());
            let mut res = Response::with((mime, status::Ok, output.bytes));



            res.headers.set_raw("X-Imageflow-Perf", vec![perf.short().into_bytes()]);
            Ok(res)
        }
        Err(e) => respond_with_server_error(&debug_info, e, true)
    }
}

fn respond_with_server_error<A>(debug_info: &A, e: ServerError, detailed_errors: bool) -> IronResult<Response> where A: std::fmt::Debug {
    match e {
        ServerError::UpstreamResponseError(reqwest::StatusCode::NOT_FOUND) => {
            let bytes = if detailed_errors {
                b"Remote file not found (upstream server responded with 404)".to_vec()
            }else {
                format!("Remote file not found (upstream server responded with 404 to {:?})", debug_info).into_bytes()
            };

            Ok(Response::with((Mime::from_str("text/plain").unwrap(),
                               status::NotFound,
                               bytes)))
        },
        e => {
            let bytes = if detailed_errors {
                format!("Internal Server Error\nInfo:{:?}\nError:{:?}", debug_info, e).into_bytes()
            }else{
                b"Internal Server Error".to_vec()
            };
            Ok(Response::with((Mime::from_str("text/plain").unwrap(),
                               status::InternalServerError,
                               bytes)))
            // TODO: get a bit more specific with the error codes
        }
    }
}


fn ir4_http_respond<F>(shared: &SharedData, url: &str, framewise_generator: F) -> IronResult<Response>
    where F: Fn(s::ImageInfo) -> std::result::Result<s::Framewise, ServerError>
{
    respond_using(&url, || fetch_bytes_using_cache_by_url(&shared.source_cache, url).map_err(error_upstream), framewise_generator)
}

fn ir4_http_respond_uncached<F>(_shared: &SharedData, url: &str, framewise_generator: F) -> IronResult<Response>
    where F: Fn(s::ImageInfo) -> std::result::Result<s::Framewise, ServerError>
{
    respond_using(&url, || {
        fetch_bytes( url, None).map_err(error_upstream).map(|r|
            (r.bytes, r.perf))
    }, framewise_generator)
}


fn ir4_framewise(_info: &s::ImageInfo, url: &url::Url) -> std::result::Result<s::Framewise, ServerError> {
    let t = ::imageflow_riapi::ir4::Ir4Translate{
        i: ::imageflow_riapi::ir4::Ir4Command::Url(url.as_str().to_owned()),
        decode_id: Some(0),
        encode_id: Some(1),
    };
    t.translate().map_err( ServerError::LayoutSizingError).and_then(|r: ::imageflow_riapi::ir4::Ir4Result| Ok(s::Framewise::Steps(r.steps.unwrap())))
}


type EngineHandler<T> = fn(req: &mut Request, engine_data: &T, mount: &MountLocation) -> IronResult<Response>;
type EngineSetup<T> = fn(mount: &MountLocation) -> Result<(T, EngineHandler<T>), String>;


fn ir4_local_respond<F>(_: &SharedData, source: &Path, framewise_generator: F) -> IronResult<Response>
    where F: Fn(s::ImageInfo) -> std::result::Result<s::Framewise, ServerError>
{
    respond_using(&source, || fetch_bytes_from_disk(source), framewise_generator)
}

fn ir4_local_handler(req: &mut Request, local_path: &PathBuf, _: &MountLocation) -> IronResult<Response> {
    let requested_path = requested_path::RequestedPath::new(local_path, req);

    let url: url::Url = req.url.clone().into();
    let shared = req.get::<persistent::Read<SharedData>>().unwrap();

    if requested_path.path.exists() {
        return ir4_local_respond(&shared, requested_path.path.as_path(), move |info: s::ImageInfo| {
            ir4_framewise(&info, &url)
        });
    }

    let _ = writeln!(&mut std::io::stderr(), "404 {:?} using local path {:?} and base {:?}", &url.path(), requested_path.path.as_path(), local_path);
    //writeln!(&mut std::io::stdout(), "404 {:?} using local path {:?}", &url.path(), original );

    Ok(Response::with((Mime::from_str("text/plain").unwrap(),
                       status::NotFound,
                       b"File not found".to_vec())))
}

fn static_handler(_: &mut Request, _: &Static, _: &MountLocation) -> IronResult<Response> {

    Ok(Response::with((Mime::from_str("text/plain").unwrap(),
                       status::InternalServerError,
                       b"Do not use".to_vec())))
}

fn ir4_local_setup(mount: &MountLocation) -> Result<(PathBuf, EngineHandler<PathBuf>), String> {
    if mount.engine_args.len() < 1 {
        Err("ir4_local requires at least one argument - the path to the physical folder it is serving".to_owned())
    } else {
        //TODO: validate path
        let local_dir = Path::new(&mount.engine_args[0]).canonicalize().map_err(|e| format!("{:?} for {:?}", e, &mount.engine_args[0]))?;
        Ok((local_dir, ir4_local_handler))
    }
}

fn static_setup(mount: &MountLocation) -> Result<(Static, EngineHandler<Static>), String> {
    if mount.engine_args.len() < 1 {
        Err("static requires at least one argument - the path to the physical folder it is serving".to_owned())
    } else {
        //TODO: validate path
        let path = Path::new(&mount.engine_args[0]).canonicalize().map_err(|e| format!("{:?}", e))?;
        let h = if mount.engine_args.len() > 1 {
            panic!("Static file cache headers not yet supported") //(we must compile staticfile with the 'cache' feature enabled)
//            let mins = mount.engine_args[1].parse::<i64>().expect("second argument to static must be the number of minutes to browser cache for");
//            Static::new(path).cache(Duration::minutes(mins))
        } else {
            Static::new(path)
        };
        Ok((h, static_handler))
    }
}

//Function is passed as generic trait (generic over 2nd arg), thus &String
#[cfg_attr(feature = "cargo-clippy", allow(ptr_arg))]
fn permacache_proxy_handler(req: &mut Request, base_url: &String, _: &MountLocation) -> IronResult<Response> {
    let url: url::Url = req.url.clone().into();
    let shared = req.get::<persistent::Read<SharedData>>().unwrap();
    //TODO: Ensure the combined url is canonical (or, at least, lacks ..)
    let remote_url = format!("{}{}{}", base_url, &url.path()[1..], req.url.query().unwrap_or(""));

    match fetch_response_using_cache_by_url(&shared.source_cache, &remote_url) {
        Ok((output, _)) => {
            let mime = output.content_type
                .parse::<Mime>()
                .unwrap_or_else(|_|Mime::from_str("application/octet-stream").unwrap());

            Ok(Response::with((mime, status::Ok, output.bytes)))
        }
        Err(e) => respond_with_server_error(&remote_url, e, true)
    }
}
lazy_static! {
    static ref MIME_TYPES: mime_types::Types = mime_types::Types::new().unwrap();
}

//Function is passed as generic trait (generic over 2nd arg), thus &String
#[cfg_attr(feature = "cargo-clippy", allow(ptr_arg))]
fn permacache_proxy_handler_guess_types(req: &mut Request, base_url: &String, _: &MountLocation) -> IronResult<Response> {

    let url: url::Url = req.url.clone().into();

    let shared = req.get::<persistent::Read<SharedData>>().unwrap();
    //TODO: Ensure the combined url is canonical (or, at least, lacks ..)
    let remote_url = format!("{}{}{}", base_url, &url.path()[1..], req.url.query().unwrap_or(""));
    match fetch_bytes_using_cache_by_url(&shared.source_cache, &remote_url) {
        Ok((bytes, _)) => {

            let part_path = Path::new(&url.path()[1..]);
            let mime_str = MIME_TYPES.mime_for_path(part_path);
            let mime:Mime  = mime_str.parse().unwrap();

//            let mime = output.content_type
//                .parse::<Mime>()
//                .unwrap_or(Mime::from_str("application/octet-stream").unwrap());

            Ok(Response::with((mime, status::Ok, bytes)))
        }
        Err(e) => respond_with_server_error(&remote_url, e, true)
    }
}

//Function is passed as generic trait (generic over 2nd arg), thus &String
#[cfg_attr(feature = "cargo-clippy", allow(ptr_arg))]
fn ir4_http_handler(req: &mut Request, base_url: &String, _: &MountLocation) -> IronResult<Response> {
    let url: url::Url = req.url.clone().into();
    let shared = req.get::<persistent::Read<SharedData>>().unwrap();
    //TODO: Ensure the combined url is canonical (or, at least, lacks ..)
    let remote_url = format!("{}{}", base_url, &url.path()[1..]);

    ir4_http_respond(&shared, &remote_url, move |info: s::ImageInfo| {
        ir4_framewise(&info, &url)
    })
}

#[cfg_attr(feature = "cargo-clippy", allow(ptr_arg))]
fn ir4_proxy_uncached_handler(req: &mut Request, base_url: &String, _: &MountLocation) -> IronResult<Response> {
    let url: url::Url = req.url.clone().into();
    let shared = req.get::<persistent::Read<SharedData>>().unwrap();
    //TODO: Ensure the combined url is canonical (or, at least, lacks ..)
    let remote_url = format!("{}{}", base_url, &url.path()[1..]);

    ir4_http_respond_uncached(&shared, &remote_url, move |info: s::ImageInfo| {
        ir4_framewise(&info, &url)
    })
}

fn ir4_http_setup(mount: &MountLocation) -> Result<(String, EngineHandler<String>), String> {
    if mount.engine_args.len() < 1 {
        Err("ir4_http requires at least one argument - the base url to suffix paths to".to_owned())
    } else {
        Ok((mount.engine_args[0].to_owned(), ir4_http_handler))
    }
}

fn ir4_http_uncached_setup(mount: &MountLocation) -> Result<(String, EngineHandler<String>), String> {
    if mount.engine_args.len() < 1 {
        Err("ir4_proxy_uncached requires at least one argument - the base url to suffix paths to".to_owned())
    } else {
        Ok((mount.engine_args[0].to_owned(), ir4_proxy_uncached_handler))
    }
}

fn permacache_proxy_setup(mount: &MountLocation) -> Result<(String, EngineHandler<String>), String> {
    if mount.engine_args.len() < 1 {
        Err("permacache_proxy requires at least one argument - the base url to suffix paths to".to_owned())
    } else {
        Ok((mount.engine_args[0].to_owned(), permacache_proxy_handler))
    }
}
fn permacache_proxy_guess_content_types_setup(mount: &MountLocation) -> Result<(String, EngineHandler<String>), String> {
    if mount.engine_args.len() < 1 {
        Err("permacache_proxy_guess_content_types requires at least one argument - the base url to suffix paths to".to_owned())
    } else {
        Ok((mount.engine_args[0].to_owned(), permacache_proxy_handler_guess_types))
    }
}

fn mount<T>(mount: MountLocation, mou: &mut mount::Mount, setup: EngineSetup<T>) -> Result<(), String>
    where T: Send, T: Sync, T: 'static {
    let (data, handler) = setup(&mount)?;

    let prefix = mount.prefix.clone();
    mou.mount(&prefix, move |r: &mut Request| { handler(r, &data, &mount) });
    Ok(())
}


pub fn serve(c: StartServerConfig) {
    env_logger::init();

    let shared_data = SharedData {
        source_cache: CacheFolder::new(c.data_dir.join(Path::new("source_cache")).as_path(), c.default_cache_layout.unwrap_or(FolderLayout::Normal)),
        output_cache: CacheFolder::new(c.data_dir.join(Path::new("output_cache")).as_path(), c.default_cache_layout.unwrap_or(FolderLayout::Normal)),
        requests_received: AtomicUsize::new(0) //NOT YET USED
    };

    let mut mou = mount::Mount::new();
    let mut router = Router::new();

    // Mount prefix (external) (url|relative path)
    // pass through static files (whitelisted??)

    for m in c.mounts {
        let copy = m.clone();
        let mount_result = match m.engine {
            //MountedEngine::Ir4Https => "ir4_https",
            MountedEngine::Ir4Http => mount(m, &mut mou, ir4_http_setup),
            MountedEngine::Ir4ProxyUncached => mount(m, &mut mou, ir4_http_uncached_setup),
            MountedEngine::Ir4Local => mount(m, &mut mou, ir4_local_setup),
            MountedEngine::PermacacheProxy => mount(m, &mut mou, permacache_proxy_setup),
            MountedEngine::PermacacheProxyGuessContentTypes => mount(m, &mut mou, permacache_proxy_guess_content_types_setup),
            MountedEngine::Static => {
                mou.mount(&m.prefix, static_setup(&m).expect("Failed to mount static directory").0);
                Ok(())
            },
        };
        if let Err(e) = mount_result{

            panic!("Failed to mount {} using engine {} ({:?})\n({:?})\nCurrent dir: {:?}", &copy.prefix, &copy.engine.to_id(), &copy, e, std::env::current_dir())
        }
    }

    if c.integration_test {
        router.get("/test/shutdown", move |_: &mut Request| -> IronResult<Response> {
            println!("Stopping server due to GET /test/shutdown");
            std::process::exit(0);

            //            Ok(Response::with((Mime::from_str("text/plain").unwrap(),
            //                               status::InternalServerError,
            //                               bytes)))
        }, "test-shutdown");
    }

    let mut chain = Chain::new(mou);

    chain.link(persistent::Read::<SharedData>::both(shared_data));

    let (logger_before, logger_after) = Logger::new(None);

    // Link logger_before as your first before middleware.
    chain.link_before(logger_before);

    // Link logger_after as your *last* after middleware.
    chain.link_after(logger_after);

    //let ssl = NativeTlsServer::new("identity.p12", "mypass").unwrap();


    println!("Listening on {}", c.bind_addr.as_str());
    if c.cert.is_some() {
        let pwd = c.cert_pwd.unwrap_or_default();
        let ssl = NativeTlsServer::new(c.cert.unwrap(), &pwd).unwrap();

        Iron::new(chain).https(c.bind_addr.as_str(), ssl).unwrap();
    }else{
        Iron::new(chain).http(c.bind_addr.as_str()).unwrap();
    }
}


#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MountedEngine {
    Ir4Local,
    Ir4Http,
    Ir4ProxyUncached,
    PermacacheProxy,
    PermacacheProxyGuessContentTypes,
    Static,
    //Ir4Https
}

impl MountedEngine {
    pub fn to_id(&self) -> &'static str {
        match *self {
            //MountedEngine::Ir4Https => "ir4_https",
            MountedEngine::Ir4Http => "ir4_http",
            MountedEngine::Ir4ProxyUncached => "ir4_proxy_uncached",
            MountedEngine::Ir4Local => "ir4_local",
            MountedEngine::PermacacheProxy => "permacache_proxy",
            MountedEngine::PermacacheProxyGuessContentTypes => "permacache_proxy_guess_content_types",
            MountedEngine::Static => "static"
        }
    }
    pub fn from_id(s: &str) -> Option<MountedEngine> {
        match s {
            "ir4_local" => Some(MountedEngine::Ir4Local),
            "ir4_http" => Some(MountedEngine::Ir4Http),
            "ir4_proxy_uncached" => Some(MountedEngine::Ir4ProxyUncached),
            "permacache_proxy" => Some(MountedEngine::PermacacheProxy),
            "permacache_proxy_guess_content_types" => Some(MountedEngine::PermacacheProxyGuessContentTypes),
            "static" => Some(MountedEngine::Static),
            //"ir4_https" => Some(MountedEngine::Ir4Https),
            _ => None
        }
    }

    pub fn id_values() -> &'static [&'static str] {
        static ID_VALUES: [&'static str; 5] = ["ir4_local", "ir4_http", "permacache_proxy", "static", "permacache_proxy_guess_content_types"/* "ir4_https"*/];

        &ID_VALUES
    }
}

trait Engine {
    fn mount(self, mount: MountLocation, router: &mut Router) -> Result<(), String>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct MountLocation {
    pub prefix: String,
    pub engine: MountedEngine,
    pub engine_args: Vec<String>,
    //TODO: HTTPS
}

impl MountLocation {
    pub fn parse(prefix: String, engine_name: String, args: Vec<String>) -> std::result::Result<MountLocation, String> {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"\A(/[a-zA-Z0-9-_]+?)+?/\z").unwrap();
        }
        if !RE.is_match(&prefix) {
            return Err("mount points must be valid paths with leading and trailing slashes, like /img/logos/. Between slashes, [a-zA-Z0-9-_] may be used".to_owned());
        }
        let engine = MountedEngine::from_id(engine_name.as_str());

        if engine.is_none() {
            return Err(format!("Valid engine names include {:?}. Provided {}", MountedEngine::id_values(), engine_name.as_str()));
        }

        Ok(MountLocation {
            prefix: prefix,
            engine: engine.unwrap(),
            engine_args: args
        })
    }
}


#[derive(Debug, Clone, PartialEq)]
pub struct StartServerConfig {
    pub data_dir: PathBuf,
    pub bind_addr: String,
    pub mounts: Vec<MountLocation>,
    pub default_cache_layout: Option<FolderLayout>,
    pub integration_test: bool,
    pub cert: Option<PathBuf>,
    pub cert_pwd: Option<String>
}


#[test]
fn test_file_macro_for_this_build(){
    assert!(file!().starts_with(env!("CARGO_PKG_NAME")))
}
