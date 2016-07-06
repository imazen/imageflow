extern crate hyper;

use hyper::*;

#[test]
fn post_backend() {
    let client = Client::new();

    let res = client.post("http://localhost:3000/set").body(r#"{ "msg": "Just trust the Rust" }"#).send().unwrap();

    assert_eq!(res.status, hyper::Ok);
}

#[test]
fn get_backend() {
    let client = Client::new();

    let res = client.get("http://localhost:3000/").send().unwrap();

    assert_eq!(res.status, hyper::Ok);
