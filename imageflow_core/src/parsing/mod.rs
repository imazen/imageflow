mod parse_graph;
mod parse_io;

use std;

extern crate rustc_serialize;
extern crate libc;

use {ContextPtr, JobPtr};
use JsonResponse;
use flow;
use libc::c_void;

use parsing::rustc_serialize::hex::FromHex;
use std::collections::HashMap;

use std::ptr;
extern crate imageflow_types as s;
extern crate serde;
extern crate serde_json;

use ::Context;

use ffi;


pub use self::parse_graph::GraphTranslator;
pub use self::parse_io::IoTranslator;
use std::error;
pub struct BuildRequestHandler {

}

#[derive(Debug)]
pub enum JsonResponseError {
    Oom(()),
    NotImplemented(()),
    Other(Box<std::error::Error>),
}

impl BuildRequestHandler {
    pub fn new() -> BuildRequestHandler {
        BuildRequestHandler {}
    }
    pub fn do_and_respond(&self,
                                          ctx: &mut ContextPtr,
                                          json: &[u8])
                                          -> Result<JsonResponse, JsonResponseError> {

        let parsed: s::Build001 = serde_json::from_slice(json).unwrap();

        let io_vec = parsed.io;

        unsafe {
            let p = ctx.ptr.unwrap();

            let mut g = GraphTranslator::new().translate_framewise(parsed.framewise);
            let job = JobPtr::create(p).unwrap();
            if let Some(s::Build001Config{ ref no_gamma_correction, ..}) = parsed.builder_config {
                ::ffi::flow_context_set_floatspace(p, match *no_gamma_correction { true => ::ffi::Floatspace::srgb, _ => ::ffi::Floatspace::linear},0f32,0f32,0f32)
            }

            if let Some(s::Build001Config{ graph_recording, ..}) = parsed.builder_config {
                if let Some(r) = graph_recording {
                    job.configure_graph_recording(r);
                }
            }

            IoTranslator::new(p).add_to_job(job.as_ptr(), io_vec);



            if !job.execute(&mut g){
                ctx.assert_ok(Some(&mut g));
            }


            // TODO: flow_job_destroy

            // TODO: Question, should JSON endpoints populate the Context error stacktrace when something goes wrong? Or populate both (except for OOM).

            Ok(JsonResponse::ok())
        }
    }
}

// #[test]
fn test_handler() {

    let input_io = s::IoObject {
        io_id: 0,
        direction: s::IoDirection::Input,

        io: s::IoEnum::BytesHex("FFD8FFE000104A46494600010101004800480000FFDB004300FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC2000B080001000101011100FFC40014100100000000000000000000000000000000FFDA0008010100013F10".to_owned())
    };

    let output_io = s::IoObject {
        io_id: 1,
        direction: s::IoDirection::Output,

        io: s::IoEnum::OutputBuffer,
    };

    let mut steps = vec![];
    steps.push(s::Node::Decode { io_id: 0, commands: None });
    steps.push(s::Node::Resample2D {
        w: 20,
        h: 30,
        down_filter: None,
        up_filter: None,
        hints: None
    });
    steps.push(s::Node::FlipV);
    steps.push(s::Node::FlipH);
    steps.push(s::Node::Rotate90);
    steps.push(s::Node::Rotate180);
    steps.push(s::Node::Rotate270);
    steps.push(s::Node::Transpose);
    steps.push(s::Node::ExpandCanvas {
        top: 2,
        left: 3,
        bottom: 4,
        right: 5,
        color: s::Color::Srgb(s::ColorSrgb::Hex("aeae22".to_owned())),
    });
    steps.push(s::Node::FillRect {
        x1: 0,
        x2: 10,
        y1: 0,
        y2: 10,
        color: s::Color::Srgb(s::ColorSrgb::Hex("ffee00".to_owned())),
    });
    steps.push(s::Node::Encode {
        io_id: 1,
        preset: s::EncoderPreset::LibjpegTurbo{ quality: Some(90)}
    });

    //    let recording = s::Build001_Graph_Recording{
    //        record_graph_versions: Some(true),
    //        record_frame_images: Some(true),
    //        render_last_graph: Some(true),
    //        render_animated_graph: Some(true),
    //        render_graph_versions : Some(true),
    //    };

    let build = s::Build001 {
        builder_config: Some(s::Build001Config {
            graph_recording: None,
            process_all_gif_frames: Some(false),
            enable_jpeg_block_scaling: Some(false),
            no_gamma_correction: false,
        }),
        io: vec![input_io, output_io],
        framewise: s::Framewise::Steps(steps),
    };

    let json_str = serde_json::to_string_pretty(&build).unwrap();

    let handler = BuildRequestHandler::new();

    let mut context = Context::create().unwrap();

    let mut ctx_cell = context.unsafe_borrow_mut_context_pointer();

    // println!("{}", json_str);

    let p = std::env::current_dir().unwrap();
    //println!("The current directory is {}", p.display());

    let response = handler.do_and_respond(&mut *ctx_cell, json_str.into_bytes().as_slice());



}
