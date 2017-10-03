use ::preludes::from_std::*;
use ::std;
use ::reqwest;
use ::hyper;
use ::hyper::Client;
use ::hyper::net::HttpsConnector;
use ::hyper_native_tls::NativeTlsClient;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
use openssl::ssl::{SslMethod, SslConnectorBuilder};
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
use hyper_openssl::OpensslClient;

#[derive(Debug)]
pub enum FetchError {
    ReqwestError(reqwest::Error),
    HyperError(hyper::Error),
    IoError(std::io::Error),
    UpstreamResponseError(hyper::status::StatusCode),

    UpstreamResponseErrorWithResponse{ status: hyper::status::StatusCode, response: FetchedResponse},
}


pub struct FetchedResponse {
    pub bytes: Vec<u8>,
    pub content_type: hyper::header::ContentType,
}

impl fmt::Debug for FetchedResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // If there is a second key/value, we're assuming it is 'charset'
        if !self.content_type.2.is_empty() || (self.content_type.0).0 == hyper::mime::TopLevel::Text{
            write!(f, "FetchedResponse {{ content_type: {:?}, length: {}, as_string: {:?} }}", self.content_type, self.bytes.len(), std::str::from_utf8(&self.bytes))
        }else{
            write!(f, "FetchedResponse {{ content_type: {:?}, length: {} }}", self.content_type, self.bytes.len())
        }
    }
}

pub fn fetch_bytes(url: &str) -> std::result::Result<Vec<u8>, FetchError> {
    fetch(url, Default::default()).map(|r| r.bytes)
}

#[derive(Default, Clone, Debug)]
pub struct FetchConfig{
    /// Only honored on linux
    pub custom_ca_trust_file: Option<PathBuf>,
    pub read_error_body: Option<bool>
}

impl FetchConfig {
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    pub fn supports_custom_ca() -> bool{
        true
    }
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    pub fn supports_custom_ca() -> bool{
        false
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    pub fn create_hyper_client(&self) -> Client{
        let mut ssl = SslConnectorBuilder::new(SslMethod::tls()).unwrap();
        if let Some(ref path) = self.custom_ca_trust_file{
            ssl.builder_mut().set_ca_file(path).unwrap();
        }
        let ssl = OpensslClient::from(ssl.build());
        let connector = HttpsConnector::new(ssl);
        Client::with_connector(connector)
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    pub fn create_hyper_client(&self) -> Client{
        let ssl = NativeTlsClient::new().unwrap();
        let connector = HttpsConnector::new(ssl);
        Client::with_connector(connector)
    }
}



pub fn fetch(url: &str, config: Option<FetchConfig>) -> std::result::Result<FetchedResponse, FetchError> {
    let conf = config.unwrap_or_default();
    let client = conf.create_hyper_client();

    let mut res = client.get(url).send()?;

    let response = if res.status == hyper::Ok || conf.read_error_body.unwrap_or(false) {
        let mut source_bytes = Vec::new();
        let _ = res.read_to_end(&mut source_bytes)?;
        Some(FetchedResponse {
            bytes: source_bytes,
            content_type: res.headers.get::<hyper::header::ContentType>().expect("content type required").clone()
        })
    } else {
        None
    };

    match (response, res.status) {
        (Some(r), hyper::Ok) => Ok(r),
        (Some(r),
            _) =>
            Err(FetchError::UpstreamResponseErrorWithResponse { status: res.status, response: r }),
        (None, _) => Err(FetchError::UpstreamResponseError(res.status))
    }
}



pub fn get_status_code_for(url: &str) -> std::result::Result<hyper::status::StatusCode, FetchError> {
    let ssl = NativeTlsClient::new().unwrap();
    let connector = HttpsConnector::new(ssl);
    let client = Client::with_connector(connector);


    let res = client.get(url).send()?;
    Ok(res.status)

    //Ok(*reqwest::get(url)?.status())
}

impl From<reqwest::Error> for FetchError {
    fn from(e: reqwest::Error) -> FetchError {
        FetchError::ReqwestError(e)
    }
}

impl From<hyper::Error> for FetchError {
    fn from(e: hyper::Error) -> FetchError {
        FetchError::HyperError(e)
    }
}

impl From<std::io::Error> for FetchError {
    fn from(e: std::io::Error) -> FetchError {
        FetchError::IoError(e)
    }
}
