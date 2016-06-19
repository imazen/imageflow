extern crate iron;
extern crate router;
extern crate rustc_serialize;
extern crate hyper;
extern crate libc;
extern crate time;

extern crate imageflow_server;
use imageflow_server::boring::*;
use imageflow_server::ffi::*;

use hyper::Client;
use iron::mime::Mime;
use iron::prelude::*;
use iron::status;
use router::Router;
use std::io::Read;
use time::precise_time_ns;


fn create_io(c: *mut Context, source_bytes: *const u8, count: usize) -> Vec<IoResource> {
    unsafe {
        let input_io = flow_io_create_from_memory(c,
                                                  IoMode::read_seekable,
                                                  source_bytes,
                                                  count as libc::size_t,
                                                  c as *mut libc::c_void,
                                                  0 as *mut libc::c_void);

        if input_io.is_null() {
            flow_context_print_and_exit_if_err(c);
            // bad, we shouldn't exit the process
        }
        let output_io = flow_io_create_for_output_buffer(c, c as *mut libc::c_void);

        if output_io.is_null() {
            flow_context_print_and_exit_if_err(c);
        }


        vec![IoResource {
                 io: input_io,
                 direction: IoDirection::In,
             },
             IoResource {
                 io: output_io,
                 direction: IoDirection::Out,
             }]
    }
}

fn collect_result(c: *mut Context, job: *mut Job) -> Result<Vec<u8>, String> {
    unsafe {
        let output_io = flow_job_get_io(c, job, 1);
        if output_io.is_null() {
            flow_context_print_and_exit_if_err(c);
        }

        let mut buf: *mut u8 = std::mem::uninitialized(); //This is okay, it's write-only
        let mut buf_length: libc::size_t = 0;
        // Returns false if the flow_io struct is disposed or not an output buffer type (or for any other error)

        if !flow_io_get_output_buffer(c, output_io, &mut buf, &mut buf_length) {
            flow_context_print_and_exit_if_err(c);
        }

        Ok(std::slice::from_raw_parts(buf as *const u8, buf_length as usize).to_vec())
    }
}


fn get_jpeg_bytes(source: &str, w: Option<u32>, h: Option<u32>) -> Vec<u8> {

    let start = precise_time_ns();

    let client = Client::new();
    let mut res = client.get(source).send().unwrap();
    assert_eq!(res.status, hyper::Ok);

    let mut source_bytes = Vec::new();
    let count = res.read_to_end(&mut source_bytes).unwrap(); //bad

    let downloaded = precise_time_ns();

    let commands = BoringCommands {
        fit: ConstraintMode::Max,
        w: w.unwrap_or(0) as i32,
        h: h.unwrap_or(0) as i32,
        jpeg_quality: 90,
        precise_scaling_ratio: 2.1f32,
        luma_correct: true,
    };


    let source_ptr = source_bytes.as_mut_ptr();

    let bytes = imageflow_server::boring::process_image(commands,
                                                        |c| create_io(c, source_ptr, count),
                                                        collect_result);

    std::mem::forget(source_bytes);

    let fetch =  downloaded - start;
    let delta = precise_time_ns() - downloaded;
    println!("HTTP fetch took: {} ms, processing took {} ms",(fetch as f64) / 1000000.0,  (delta as f64) / 1000000.0);

    return bytes.unwrap();



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
               move |r: &mut Request| proto1(r));

    Iron::new(router).http("localhost:3000").unwrap();
}
