//! Imageflow jobs on small host thread stacks.
//!
//! Imageflow runs on the caller's thread. IIS reserves 512 KB of stack per worker
//! thread on x64 and 256 KB on x86, and Imageflow.Net calls into imageflow on the
//! request thread. A native stack overflow there cannot be caught: it terminates the
//! worker process, and IIS disables the application pool after repeated crashes
//! (imageflow#737: building a LUT color transform put a 786,432-byte table on the
//! stack). These tests run whole jobs on a 256 KB thread.
//!
//! Color transforms are cached per process, so these tests rely on nextest running
//! each test in its own process: under `cargo test`, another test can create and
//! cache a transform first.

use crate::common::get_url_bytes_with_retry;
use imageflow_core::Context;
use imageflow_types as s;

/// The x86 IIS worker-thread stack reservation, the smallest host stack we support.
const SMALL_HOST_STACK_BYTES: usize = 256 * 1024;

/// Decode `url`, resize it to 100x100 and re-encode it on a thread with a
/// [`SMALL_HOST_STACK_BYTES`] stack. A stack overflow aborts the test process.
fn resize_on_small_stack(url: &str) {
    let bytes = get_url_bytes_with_retry(url).unwrap_or_else(|e| panic!("fetching {url}: {e}"));
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("small-host-stack".to_owned())
            .stack_size(SMALL_HOST_STACK_BYTES)
            .spawn_scoped(scope, move || {
                let mut ctx = Context::create().unwrap();
                ctx.add_input_vector(0, bytes).unwrap();
                ctx.add_output_buffer(1).unwrap();
                ctx.execute_1(s::Execute001 {
                    job_options: None,
                    graph_recording: Some(s::Build001GraphRecording::off()),
                    security: None,
                    framewise: s::Framewise::Steps(vec![s::Node::CommandString {
                        kind: s::CommandStringKind::ImageResizer4,
                        value: "width=100&height=100".to_owned(),
                        decode: Some(0),
                        encode: Some(1),
                        watermarks: None,
                    }]),
                })
                .unwrap();
            })
            .expect("spawning the small-stack thread")
            .join()
            .expect("the job panicked on the small-stack thread");
    });
}

/// CMYK JPEG without an embedded profile: converted to sRGB with the bundled SWOP
/// profile through a CMYK LUT transform.
#[test]
fn cmyk_jpeg_on_small_stack() {
    resize_on_small_stack(
        "https://imageflow-resources.s3-us-west-2.amazonaws.com/test_inputs/cmyk_logo.jpg",
    );
}

/// RGB JPEG whose ICC v4 profile has only A2B LUT tags, no matrix/TRC, so its
/// transform is a 3D LUT.
#[test]
fn rgb_jpeg_with_lut_profile_on_small_stack() {
    resize_on_small_stack(
        "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_v4_sYCC_8bit.jpg",
    );
}

/// Control: RGB JPEG with a matrix/TRC profile, whose transform needs no LUT.
#[test]
fn rgb_jpeg_with_matrix_profile_on_small_stack() {
    resize_on_small_stack(
        "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/MarsRGB_tagged.jpg",
    );
}
