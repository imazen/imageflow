
extern crate hyper_native_tls;
extern crate imageflow_helpers;

use std;
use std::fmt;
use reqwest;
use hyper;
use reqwest::{Client, Certificate};
use ::imageflow_helpers::filesystem::read_file_bytes;
use std::path::PathBuf;
use std::io::Read;

#[derive(Debug)]
pub enum FetchError {
    ReqwestError(reqwest::Error),
    HyperError(hyper::Error),
    IoError(std::io::Error),
    UpstreamResponseError(reqwest::StatusCode),

    UpstreamResponseErrorWithResponse{ status: reqwest::StatusCode, response: FetchedResponse},
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FetchError::ReqwestError(ref e) => e.fmt(f),
            FetchError::HyperError(ref e) => e.fmt(f),
            FetchError::IoError(ref e) => e.fmt(f),
            FetchError::UpstreamResponseError(ref status) |
            FetchError::UpstreamResponseErrorWithResponse {ref status, ..} => {
                write!(f, "Response status {}", status)
            },
        }
    }
}

pub type FetchResult = ::std::result::Result<FetchedResponse,FetchError>;

pub struct FetchedResponse {
    pub bytes: Vec<u8>,
    pub content_type: reqwest::header:: HeaderValue,
}

impl fmt::Debug for FetchedResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // If there is a second key/value, we're assuming it is 'charset'
        if !self.content_type.is_empty() || self.content_type.to_str().unwrap() == "text/plain"{
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
    /// Only honored on linux (maybe outdated?)
    /// PEM format
    pub custom_ca_trust_file: Option<PathBuf>,
    pub read_error_body: Option<bool>
}

impl FetchConfig {
//    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
//    pub fn supports_custom_ca() -> bool{
//        true
//    }
//    #[cfg(any(target_os = "windows", target_os = "macos"))]
//    pub fn supports_custom_ca() -> bool{
//        false
//    }
//    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
//    pub fn build_client(&self) -> Client{
//        let mut ssl = SslConnectorBuilder::new(SslMethod::tls()).unwrap();
//        if let Some(ref path) = self.custom_ca_trust_file{
//            ssl.set_ca_file(path).unwrap();
//        }
//        let ssl = OpensslClient::from(ssl.build());
//        let connector = HttpsConnector::new(ssl);
//        Client::with_connector(connector)
//    }
//
//    #[cfg(any(target_os = "windows", target_os = "macos"))]
//    pub fn build_client(&self) -> Client{
//        let ssl = NativeTlsClient::new().unwrap();
//        let connector = HttpsConnector::new(ssl);
//        Client::with_connector(connector)
//    }

    pub fn build_client(&self) -> Client{
        let builder = if let Some(ref cert) = self.custom_ca_trust_file{
            let bytes = read_file_bytes(cert).unwrap();
            reqwest::ClientBuilder::new().add_root_certificate(Certificate::from_pem(&bytes).unwrap())
        } else{
            reqwest::ClientBuilder::new()
        };
        builder.build().unwrap()
    }
}



pub fn fetch(url: &str, config: Option<FetchConfig>) -> std::result::Result<FetchedResponse, FetchError> {
    let conf = config.unwrap_or_default();
    let client = conf.build_client();

    let mut res = client.get(url).send()?;

    let response = if res.status().is_success() || conf.read_error_body.unwrap_or(false) {
        let mut source_bytes = Vec::new();
        let _ = res.read_to_end(&mut source_bytes)?;
        Some(FetchedResponse {
            bytes: source_bytes,
            content_type: res.headers().get(reqwest::header::CONTENT_TYPE).expect("content type required").clone()
        })
    } else {
        None
    };

    if res.status().is_success() && response.is_some(){
        Ok(response.unwrap())
    }else {
        match (response, res.status()) {
            (Some(r),
                _) =>
                Err(FetchError::UpstreamResponseErrorWithResponse { status: res.status(), response: r }),
            (None, _) => Err(FetchError::UpstreamResponseError(res.status()))
        }
    }
}



pub fn get_status_code_for(url: &str) -> std::result::Result<reqwest::StatusCode, FetchError> {

    let client = reqwest::ClientBuilder::new().use_default_tls().build().unwrap();

    let res = client.get(url).send()?;
    Ok(res.status())

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
