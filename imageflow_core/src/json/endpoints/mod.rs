pub(crate) mod v1;
use super::parse_json;
use crate::internal_prelude::works_everywhere::*;
use crate::json::*;
use crate::parsing::GraphTranslator;
use crate::parsing::IoTranslator;
use crate::Context;
use imageflow_types::*;
use serde::{Deserialize, Serialize};
use std::error;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn <T>(context: &mut Context, method: &str, json: &[u8], poll_cancellation: Option<trait Fn() -> bool>) -> Result<JsonResponse> {
    let redirect = match method {
        "v0.1/build" => "v1/build",
        "v0.1/get_image_info" => "v1/get_image_info",
        "v0.1/get_scaled_image_info" => "v1/get_scaled_image_info",
        "v0.1/tell_decoder" => "v1/tell_decoder",
        "v0.1/execute" => "v1/execute",
        "v0.1/get_version_info" => "v1/get_version_info",
        "brew_coffee" => "v1/brew_coffee",
        _ => method,
    };
    v1::invoke(context, redirect, json, poll_cancellation)
}
pub fn try_invoke_static(method: &str, json: &[u8]) -> Result<Option<JsonResponse>> {
    let redirect = match method {
        "v0.1/get_version_info" => "v1/get_version_info",
        "brew_coffee" => "v1/brew_coffee",
        _ => method,
    };
    v1::try_invoke_static(redirect, json)
}

fn get_create_doc_dir() -> std::path::PathBuf {
    let path = ::imageflow_types::version::crate_parent_folder().join(Path::new("target/doc"));
    let _ = std::fs::create_dir_all(&path);
    // Error { repr: Os { code: 17, message: "File exists" } }
    // The above can happen, despite the docs.
    path
}
#[test]
fn write_context_doc() {
    let path = get_create_doc_dir().join(Path::new("context_json_api.txt"));
    File::create(&path).unwrap().write_all(document_message().as_bytes()).unwrap();
}

fn document_message() -> String {
    let mut s = String::new();
    s.reserve(8000);
    s += "# JSON API - Context\n\n";
    s += "imageflow_context responds to these message methods\n\n";
    s += "## v1/build \n";
    s += "Example message body:\n";
    s += &serde_json::to_string_pretty(&Build001::example_with_steps()).unwrap();
    s += "\n\nExample response:\n";
    s += &serde_json::to_string_pretty(&Response001::example_job_result_encoded(
        2,
        200,
        200,
        "image/png",
        "png",
    ))
    .unwrap();
    s += "## v1/get_image_info \n";
    s += "Example message body:\n";
    s += &serde_json::to_string_pretty(&GetImageInfo001::example_get_image_info()).unwrap();
    s += "\nExample response:\n";
    s += &serde_json::to_string_pretty(&Response001::example_image_info()).unwrap();
    s += "\n\n";

    s += "## v1/tell_decoder \n";
    s += "Example message body:\n";
    s += &serde_json::to_string_pretty(&TellDecoder001::example_hints()).unwrap();
    s += "\nExample response:\n";
    s += &serde_json::to_string_pretty(&Response001::example_ok()).unwrap();
    s += "\n\n";

    s += "## v1/execute \n";
    s += "Example message body (with graph):\n";
    s += &serde_json::to_string_pretty(&Execute001::example_graph()).unwrap();
    s += "Example message body (with linear steps):\n";
    s += &serde_json::to_string_pretty(&Execute001::example_steps()).unwrap();
    s += "\nExample response:\n";
    s += &serde_json::to_string_pretty(&Response001::example_job_result_encoded(
        2,
        200,
        200,
        "image/jpg",
        "jpg",
    ))
    .unwrap();
    s += "\nExample failure response:\n";
    s += &serde_json::to_string_pretty(&Response001::example_error()).unwrap();
    s += "\n\n";

    s
}

// #[test]
fn test_handler() {
    let input_io = IoObject {
        io_id: 0,
        direction: IoDirection::In,

        io: IoEnum::BytesHex("FFD8FFE000104A46494600010101004800480000FFDB004300FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC2000B080001000101011100FFC40014100100000000000000000000000000000000FFDA0008010100013F10".to_owned())
    };

    let output_io = IoObject { io_id: 1, direction: IoDirection::Out, io: IoEnum::OutputBuffer };

    let mut steps = vec![];
    steps.push(Node::Decode { io_id: 0, commands: None });
    steps.push(Node::Resample2D { w: 20, h: 30, hints: None });
    steps.push(Node::FlipV);
    steps.push(Node::FlipH);
    steps.push(Node::Rotate90);
    steps.push(Node::Rotate180);
    steps.push(Node::Rotate270);
    steps.push(Node::Transpose);
    steps.push(Node::ExpandCanvas {
        top: 2,
        left: 3,
        bottom: 4,
        right: 5,
        color: Color::Srgb(ColorSrgb::Hex("aeae22".to_owned())),
    });
    steps.push(Node::FillRect {
        x1: 0,
        x2: 10,
        y1: 0,
        y2: 10,
        color: Color::Srgb(ColorSrgb::Hex("ffee00".to_owned())),
    });
    steps.push(Node::Encode {
        io_id: 1,
        preset: EncoderPreset::LibjpegTurbo {
            quality: Some(90),
            optimize_huffman_coding: None,
            progressive: None,
            matte: None,
        },
    });

    let build = Build001 {
        builder_config: Some(Build001Config {
            graph_recording: None,
            security: None,
            //            process_all_gif_frames: Some(false),
            //            enable_jpeg_block_scaling: Some(false)
        }),
        io: vec![input_io, output_io],
        framewise: Framewise::Steps(steps),
    };
    // This test is outdated as build_1 is deprecated in favor of handle_build/build_1_raw
    // let response = Context::create().unwrap().build_1(build);
}

#[test]
fn test_get_version_info() {
    let response = Context::create().unwrap().get_version_info().unwrap();

    assert!(response.build_date.len() > 0);
    assert!(response.git_describe_always.len() > 0);
    assert!(response.last_git_commit.len() > 0);
    assert!(response.long_version_string.len() > 0);
}
