extern crate imageflow_helpers;

use std::fmt;
use std::str;
mod shellout;
#[cfg(feature = "ureq")]
mod ureq;
pub use shellout::ShellFetcher;

/// Trait defining the interface for HTTP fetching implementations
pub trait HttpFetcher {
    fn fetch(&self, url: &str, config: Option<FetchConfig>) -> FetchResult;
    fn fetch_bytes(&self, url: &str) -> std::result::Result<Vec<u8>, FetchError> {
        self.fetch(url, None).map(|r| r.bytes)
    }
    fn get_status_code(&self, url: &str) -> std::result::Result<u16, FetchError>;
}

pub fn default_fetcher() -> impl HttpFetcher {
    #[cfg(feature = "ureq")]
    {
        crate::ureq::UreqFetcher::new()
    }
    #[cfg(not(feature = "ureq"))]
    {
        crate::shellout::ShellFetcher::new()
    }
}

// Create convenience functions that use the default implementation
pub fn fetch_bytes(url: &str) -> std::result::Result<Vec<u8>, FetchError> {
    default_fetcher().fetch_bytes(url)
}

pub fn fetch(url: &str, config: Option<FetchConfig>) -> FetchResult {
    default_fetcher().fetch(url, config)
}

pub fn get_status_code_for(url: &str) -> std::result::Result<u16, FetchError> {
    default_fetcher().get_status_code(url)
}

#[derive(Debug)]
pub enum FetchError {
    ToolError(String),
    IoError(std::io::Error),
    UpstreamResponseError(u16),
    ContentLengthMismatch,

    UpstreamResponseErrorWithResponse{ status: u16, response: FetchedResponse},
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FetchError::ToolError(ref e) => write!(f, "ToolError: {:?}", e),
            FetchError::IoError(ref e) => write!(f, "IoError: {:?}", e),
            FetchError::UpstreamResponseError(ref status) |
            FetchError::UpstreamResponseErrorWithResponse {ref status, ..} => {
                write!(f, "Response status {}", status)
            },
            FetchError::ContentLengthMismatch => write!(f, "Content-Length value did not match bytes received.")
        }
    }
}

pub type FetchResult = ::std::result::Result<FetchedResponse,FetchError>;

pub struct FetchedResponse {
    pub code: u16,
    pub bytes: Vec<u8>,
    pub content_type: String,
}

impl fmt::Debug for FetchedResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if !self.content_type.is_empty() || self.content_type == "text/plain" {
            write!(f, "FetchedResponse {{ content_type: {:?}, length: {}, as_string: {:?} }}", self.content_type, self.bytes.len(), std::str::from_utf8(&self.bytes))
        }else{
            write!(f, "FetchedResponse {{ content_type: {:?}, length: {} }}", self.content_type, self.bytes.len())
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct FetchConfig{

    pub read_error_body: Option<bool>
}



impl From<std::io::Error> for FetchError {
    fn from(e: std::io::Error) -> FetchError {
        FetchError::IoError(e)
    }
}


