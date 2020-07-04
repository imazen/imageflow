use crate::Context;
use crate::internal_prelude::works_everywhere::*;
use crate::json::*;
use crate::parsing::GraphTranslator;
use crate::parsing::IoTranslator;
use std::error;


fn create_context_router() -> MethodRouter<'static, Context> {
    let mut r = MethodRouter::new();
    //    r.add_responder("v0.1/load_image_info", Box::new(
    //        move |context: &mut Context, data: s::GetImageInfo001| {
    //            Ok(JsonResponse::method_not_understood())
    //            //Ok(s::ResponsePayload::ImageInfo(job.get_image_info(data.io_id)?))
    //        }
    //    ));

    r.add_responder("v0.1/build",
                    Box::new(move |context: &mut Context, parsed: s::Build001| {
                        context.build_1(parsed).map_err(|e| e.at(here!()))
                    }));
    r.add_responder("v1/build",
                    Box::new(move |context: &mut Context, parsed: s::Build001| {
                        context.build_1(parsed).map_err(|e| e.at(here!()))
                    }));
    r.add_responder("v0.1/get_image_info",
                    Box::new(move |context: &mut Context, data: s::GetImageInfo001| {
                        Ok(s::ResponsePayload::ImageInfo(context.get_unscaled_image_info(data.io_id).map_err(|e| e.at(here!()))?))
                    }));
    r.add_responder("v1/get_image_info",
                    Box::new(move |context: &mut Context, data: s::GetImageInfo001| {
                        Ok(s::ResponsePayload::ImageInfo(context.get_unscaled_image_info(data.io_id).map_err(|e| e.at(here!()))?))
                    }));
    r.add_responder("v1/get_scaled_image_info",
                    Box::new(move |context: &mut Context, data: s::GetImageInfo001| {
                        Ok(s::ResponsePayload::ImageInfo(context.get_scaled_image_info(data.io_id).map_err(|e| e.at(here!()))?))
                    }));
    r.add_responder("v0.1/tell_decoder",
                    Box::new(move |context: &mut Context, data: s::TellDecoder001| {
                        context.tell_decoder(data.io_id, data.command).map_err(|e| e.at(here!()))?;
                        Ok(s::ResponsePayload::None)
                    }));
    r.add_responder("v1/tell_decoder",
                    Box::new(move |context: &mut Context, data: s::TellDecoder001| {
                        context.tell_decoder(data.io_id, data.command).map_err(|e| e.at(here!()))?;
                        Ok(s::ResponsePayload::None)
                    }));
    r.add_responder("v0.1/execute",
                    Box::new(move |context: &mut Context, parsed: s::Execute001| {
                        context.execute_1(parsed).map_err(|e| e.at(here!()))
                    }));
    r.add_responder("v1/execute",
                    Box::new(move |context: &mut Context, parsed: s::Execute001| {
                        context.execute_1(parsed).map_err(|e| e.at(here!()))
                    }));
    r.add_responder("v1/get_version_info",
                    Box::new(move |context: &mut Context, data: s::GetVersionInfo| {
                        Ok(s::ResponsePayload::VersionInfo(context.get_version_info().map_err(|e| e.at(here!()))?))
                    }));
    r.add("brew_coffee",
          Box::new(move |context: &mut Context, bytes: &[u8]| (JsonResponse::teapot(), Ok(()))));
    r
}

lazy_static! {
        pub static ref  CONTEXT_ROUTER: MethodRouter<'static, Context> = create_context_router();
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
    s += &serde_json::to_string_pretty(&s::Build001::example_with_steps()).unwrap();
    s += "\n\nExample response:\n";
    s += &serde_json::to_string_pretty(&s::Response001::example_job_result_encoded(2,
                                                                                   200,
                                                                                   200,
                                                                                   "image/png",
                                                                                   "png"))
        .unwrap();
    s += "## v1/get_image_info \n";
    s += "Example message body:\n";
    s += &serde_json::to_string_pretty(&s::GetImageInfo001::example_get_image_info()).unwrap();
    s += "\nExample response:\n";
    s += &serde_json::to_string_pretty(&s::Response001::example_image_info()).unwrap();
    s += "\n\n";


    s += "## v1/tell_decoder \n";
    s += "Example message body:\n";
    s += &serde_json::to_string_pretty(&s::TellDecoder001::example_hints()).unwrap();
    s += "\nExample response:\n";
    s += &serde_json::to_string_pretty(&s::Response001::example_ok()).unwrap();
    s += "\n\n";

    s += "## v1/execute \n";
    s += "Example message body (with graph):\n";
    s += &serde_json::to_string_pretty(&s::Execute001::example_graph()).unwrap();
    s += "Example message body (with linear steps):\n";
    s += &serde_json::to_string_pretty(&s::Execute001::example_steps()).unwrap();
    s += "\nExample response:\n";
    s += &serde_json::to_string_pretty(&s::Response001::example_job_result_encoded(2,
                                                                                   200,
                                                                                   200,
                                                                                   "image/jpg",
                                                                                   "jpg"))
        .unwrap();
    s += "\nExample failure response:\n";
    s += &serde_json::to_string_pretty(&s::Response001::example_error()).unwrap();
    s += "\n\n";

    s
}

// #[test]
fn test_handler() {

    let input_io = s::IoObject {
        io_id: 0,
        direction: s::IoDirection::In,

        io: s::IoEnum::BytesHex("FFD8FFE000104A46494600010101004800480000FFDB004300FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC2000B080001000101011100FFC40014100100000000000000000000000000000000FFDA0008010100013F10".to_owned())
    };

    let output_io = s::IoObject {
        io_id: 1,
        direction: s::IoDirection::Out,

        io: s::IoEnum::OutputBuffer,
    };

    let mut steps = vec![];
    steps.push(s::Node::Decode {
        io_id: 0,
        commands: None,
    });
    steps.push(s::Node::Resample2D {
        w: 20,
        h: 30,
        hints: None,
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
        preset: s::EncoderPreset::LibjpegTurbo { quality: Some(90), optimize_huffman_coding: None, progressive: None },
    });

    let build = s::Build001 {
        builder_config: Some(s::Build001Config {
            graph_recording: None,
            security:None,
//            process_all_gif_frames: Some(false),
//            enable_jpeg_block_scaling: Some(false)
        }),
        io: vec![input_io, output_io],
        framewise: s::Framewise::Steps(steps),
    };

    let response = Context::create().unwrap().build_1(build);
}
