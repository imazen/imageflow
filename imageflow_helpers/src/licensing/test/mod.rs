mod strings;
mod support;

use self::strings::*;
use self::support::*;
use super::*;
//use super::cache::*;
use super::compute::*;
//use super::license_pair::*;
//use super::parsing::*;
//use super::support::*;

use mockito::mock;

use smallvec::SmallVec;


//#[cfg(not(test))]
//const URL: &'static str = "https://api.twitter.com";
//
//#[cfg(test)]
//const URL: &'static str = mockito::SERVER_URL;
//
//
//let _m = mock("GET", "/hello")
//.with_status(201)
//.with_header("content-type", "text/plain")
//.with_header("x-api-key", "1234")
//.with_body("world")
//.create();

const TWENTY_HOURS: i64 = 60 * 60 * 20;

fn mock_plaintext_200(path: &'static str, body: &'static str) -> ::mockito::Mock{
    mock("GET", path).with_status(200).with_header("content-type", "text/plain").with_body(body).create()
}

#[test]
fn test_remote_license_success(){

    let req_features = SmallVec::from_buf(["R_Creative"]);

    let mock = mock_plaintext_200("/v1/licenses/latest/testda42e8a40db14c091dea84efd572933fdfe31ba9620e5fee79edb823a448b6e8.txt", SITE_WIDE_REMOTE);

    let clock = Box::new(OffsetClock::new("2017-04-25", "2017-04-25"));
    let cache = StringMemCache::new().into_cache();
    let mgr = Arc::new(LicenseManagerSingleton::new(&*parsing::TEST_KEYS, clock, cache).rewind_boot_time(TWENTY_HOURS));

    assert!(!mgr.compute_feature("R_Creative").licensed());

    let _license = mgr.get_or_add(&Cow::Borrowed(SITE_WIDE_PLACEHOLDER)).unwrap();

    LicenseManagerSingleton::create_thread(mgr.clone());
    mgr.wait_for(1);

    assert!(mgr.compute_feature("R_Creative").licensed());

    mock.assert();
}


#[test]
fn test_remote_license_void(){
    let mock = mock_plaintext_200("/v1/licenses/latest/test8b47045eb7b8ca42aa967f33ee1d014ba89f8d1ac207426b482d34b5c0d90935.txt", CANCELLED_REMOTE);

    let clock = Box::new(OffsetClock::new("2017-04-25", "2017-04-25"));

    let cache = StringMemCache::new().into_cache();
    let mgr = Arc::new(LicenseManagerSingleton::new(&*parsing::TEST_KEYS, clock, cache).rewind_boot_time(TWENTY_HOURS));

    // Not licensed before placeholder
    assert!(!mgr.compute_feature("R_Creative").licensed());

    mgr.add_static(CANCELLED_PLACEHOLDER).unwrap();

    // Not licensed after placeholder
    //let c = mgr.compute_feature("R_Creative");
//    eprintln!("{}", c.get_diagnostics());
    assert!(!mgr.compute_feature("R_Creative").licensed());


    LicenseManagerSingleton::create_thread(mgr.clone());
    mgr.wait_for(1);

    // Not licensed after remote is fetched
    assert!(!mgr.compute_feature("R_Creative").licensed());

    mock.assert();
}


