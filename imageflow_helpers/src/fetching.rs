use ::preludes::from_std::*;
use ::hyper::{Client};
use ::hyper;
use ::std;

#[derive(Debug)]
pub enum FetchError {
    HyperError(hyper::Error),
    IoError(std::io::Error),
    UpstreamResponseError(hyper::status::StatusCode),
}

pub fn fetch_bytes(url: &str) -> std::result::Result<Vec<u8>, FetchError> {
    let client = Client::new(); //default idle connections max of 5, follow all redirects.
    let mut res = client.get(url).send()?;
    if res.status != hyper::Ok {
        return Err(FetchError::UpstreamResponseError(res.status));
    }
    let mut source_bytes = Vec::new();
    let _ = res.read_to_end(&mut source_bytes)?;
    Ok(source_bytes)
}

pub fn get_status_code_for(url: &str) -> std::result::Result<hyper::status::StatusCode, FetchError> {
    let client = Client::new(); //default idle connections max of 5, follow all redirects.
    let mut res = client.get(url).send()?;
    Ok(res.status)
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
