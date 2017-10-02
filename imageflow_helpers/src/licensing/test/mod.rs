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

use ::smallvec::SmallVec;


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



#[test]
fn test_remote_license_success(){

    let req_features = SmallVec::from_buf(["R_Creative"]);

    let mock = mock("GET", "/v1/licenses/latest/testda42e8a40db14c091dea84efd572933fdfe31ba9620e5fee79edb823a448b6e8.txt").with_status(200).with_header("content-type", "text/plain").with_body(SITE_WIDE_REMOTE).create();

    let clock = Box::new(OffsetClock::new("2017-04-25", "2017-04-25"));
    let cache = StringMemCache::new().into_cache();
    let mut mgr =LicenseManagerSingleton::new(&*parsing::TEST_KEYS, clock, cache);
    mgr.rewind_created_date(60 * 60 * 20);

    let mgr =  Arc::new(mgr);

    assert!(!mgr.compute(true, LicenseScope::All,&req_features ).licensed());

    let _license = mgr.get_or_add(&Cow::Borrowed(SITE_WIDE_PLACEHOLDER)).unwrap();

    LicenseManagerSingleton::create_thread(mgr.clone());
    mgr.wait_for(1);



    let compute = mgr.compute(true, LicenseScope::All,&req_features );

    assert!(compute.licensed());

    mock.assert();
}


