extern crate iron;
extern crate router;
extern crate rustc_serialize;
extern crate hyper;
extern crate libc;
extern crate time;

extern crate imageflow_core;

use imageflow_core::clients::stateless;

use hyper::Client;
use imageflow_core::boring::*;
use imageflow_core::ffi::*;
use iron::mime::Mime;
use iron::prelude::*;
use iron::status;
use router::Router;
use std::io::Read;
use time::precise_time_ns;

//Todo: consider lru_cache crate


//TODO: Convert parameters into Nodes
//Implement content-type export from job/execute endpoint

fn get_jpeg_bytes(source: &str, w: Option<u32>, h: Option<u32>) -> Vec<u8> {

    let start = precise_time_ns();

    let client = Client::new();
    let mut res = client.get(source).send().unwrap();
    assert_eq!(res.status, hyper::Ok);

    let mut source_bytes = Vec::new();
    let _ = res.read_to_end(&mut source_bytes).unwrap(); //bad

    let downloaded = precise_time_ns();

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

    let mut client = stateless::LibClient{};
    let info = client.get_image_info(&source_bytes).unwrap();

    let (framewise, (pre_w, pre_h)) = create_framewise(info.frame0_width, info.frame0_height, commands).unwrap();


    let result: stateless::BuildSuccess = client.build(stateless::BuildRequest{
        framewise: framewise,
        inputs: vec![stateless::BuildInput{io_id: 0, bytes: &source_bytes}],
        output_ids: vec![1]
    }).unwrap();

    let bytes = result.outputs.into_iter().next().unwrap().bytes;

    let fetch = downloaded - start;
    let delta = precise_time_ns() - downloaded;
    println!("HTTP fetch took: {} ms, processing took {} ms",
             (fetch as f64) / 1000000.0,
             (delta as f64) / 1000000.0);

    return bytes;



}

fn proto1(req: &mut Request) -> IronResult<Response> {
    let content_type = "image/jpeg".parse::<Mime>().unwrap();

    let w = req.extensions.get::<Router>().unwrap().find("w").and_then(|x| x.parse::<u32>().ok());
    let h = req.extensions.get::<Router>().unwrap().find("h").and_then(|x| x.parse::<u32>().ok());
    let url = "http://images.unsplash.com/".to_string() +
              req.extensions.get::<Router>().unwrap().find("url").unwrap();

    let payload = get_jpeg_bytes(&url, w, h);

    Ok(Response::with((content_type, status::Ok, payload)))
}


fn main() {
    let mut router = Router::new();

    router.get("/proto1/scale_unsplash_jpeg/:w/:h/:url",
               move |r: &mut Request| proto1(r),
               "proto1-unsplash");

    Iron::new(router).http("localhost:3000").unwrap();
}
