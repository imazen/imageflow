#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate imageflow_core;
extern crate imageflow_types as s;
extern crate imageflow_helpers as hlp;
extern crate serde_json;
extern crate smallvec;

pub mod common;
use crate::common::*;


use imageflow_core::{Context, FlowError, CodeLocation};
use s::{CommandStringKind, ResponsePayload};


const DEBUG_GRAPH: bool = false;
const FRYMIRE_URL: &'static str = "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/frymire.png";


#[test]
fn test_encode_png() {
    let steps = reencode_with(s::EncoderPreset::Lodepng {
        maximum_deflate: None
    });

    compare_encoded_to_source(IoTestEnum::Url(FRYMIRE_URL.to_owned()),
                              DEBUG_GRAPH,
                              Constraints {
                                  max_file_size: Some(390_000),
                                  similarity: Similarity::AllowDssimMatch(0.0, 0.0),
                              },
                              steps
    );
}


#[test]
fn test_encode_pngquant() {
    let steps = reencode_with(s::EncoderPreset::Pngquant {
                speed: None,
                quality: Some(100),
                maximum_deflate: None,
        minimum_quality: None
    });

    compare_encoded_to_source(IoTestEnum::Url(FRYMIRE_URL.to_owned()),
                              DEBUG_GRAPH,
                              Constraints {
                                  max_file_size: Some(280_000),
                                  similarity: Similarity::AllowDssimMatch(0.0017, 0.008),
                              },
                              steps
    );
}
#[test]
fn test_encode_pngquant_command() {
    let steps = reencode_with_command("png.min_quality=0&png.quality=100");

    compare_encoded_to_source(IoTestEnum::Url(FRYMIRE_URL.to_owned()),
                              DEBUG_GRAPH,
                              Constraints {
                                  max_file_size: Some(280_000),
                                  similarity: Similarity::AllowDssimMatch(0.0017, 0.008),
                              },
                              steps
    );
}
#[test]
fn test_encode_pngquant_fallback() {
    let steps = reencode_with(s::EncoderPreset::Pngquant {
                speed: None,
                quality: Some(100),
                maximum_deflate: None,
        minimum_quality: Some(99)
    });

    compare_encoded_to_source(IoTestEnum::Url(FRYMIRE_URL.to_owned()),
                              DEBUG_GRAPH,
                              Constraints {
                                  max_file_size: None,
                                  similarity: Similarity::AllowDssimMatch(0.000, 0.001),
                              },
                              steps
    );
}
#[test]
fn test_encode_pngquant_fallback_command() {
    let steps =  reencode_with_command("png.min_quality=99&png.quality=100");

    compare_encoded_to_source(IoTestEnum::Url(FRYMIRE_URL.to_owned()),
                              DEBUG_GRAPH,
                              Constraints {
                                  max_file_size: None,
                                  similarity: Similarity::AllowDssimMatch(0.000, 0.001),
                              },
                              steps
    );
}

#[test]
fn test_encode_lodepng() {
    let steps = reencode_with(s::EncoderPreset::Lodepng{
        maximum_deflate: None
    });

    compare_encoded_to_source(IoTestEnum::Url(FRYMIRE_URL.to_owned()),
                              DEBUG_GRAPH,
                              Constraints {
                                  max_file_size: Some(390_000),
                                  similarity: Similarity::AllowDssimMatch(0., 0.),
                              },
                              steps
    );
}

#[test]
fn test_encode_mozjpeg_resized() {
    let use_hermite = s::ResampleHints::new().with_bi_filter(s::Filter::Hermite);
    let steps = vec![
        s::Node::Decode { io_id: 0, commands: None },
        s::Node::Resample2D{ w: 550, h: 550, hints: Some(use_hermite.clone())},
        s::Node::Resample2D{ w: 1118, h: 1105, hints: Some(use_hermite.clone()) },
        s::Node::Encode {
            io_id: 1,
            preset: s::EncoderPreset::Mozjpeg {
                progressive: None,
                quality: Some(50),
                matte: None
            },
        },
    ];

    compare_encoded_to_source(IoTestEnum::Url(FRYMIRE_URL.to_owned()),
                              DEBUG_GRAPH,
                              Constraints {
                                  max_file_size: Some(160_000),
                                  similarity: Similarity::AllowDssimMatch(0.04, 0.2),
                              },
                              steps
    );
}

#[test]
fn test_encode_mozjpeg() {
    let steps = reencode_with(s::EncoderPreset::Mozjpeg {
                progressive: None,
                quality: Some(50),
                matte: None
            });

    compare_encoded_to_source(IoTestEnum::Url(FRYMIRE_URL.to_owned()),
                              DEBUG_GRAPH,
                              Constraints {
                                  max_file_size: Some(301_000),
                                  similarity: Similarity::AllowDssimMatch(0.007, 0.06),
                              },
                              steps
    );
}

#[test]
fn test_encode_webp_lossless() {
    let steps = reencode_with(s::EncoderPreset::WebPLossless);

    compare_encoded_to_source(IoTestEnum::Url(FRYMIRE_URL.to_owned()),
                              DEBUG_GRAPH,
                              Constraints {
                                  max_file_size: Some(301_000),
                                  similarity: Similarity::AllowDssimMatch(0., 0.),
                              },
                              steps
    );
}

#[test]
fn test_roundtrip_webp_lossless() {
    let steps = reencode_with(s::EncoderPreset::WebPLossless);

    compare_encoded_to_source(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/5_webp_ll.webp".to_owned()),
                              DEBUG_GRAPH,
                              Constraints {
                                  max_file_size: Some(301_000),
                                  similarity: Similarity::AllowDssimMatch(0., 0.),
                              },
                              steps
    );
}

#[test]
fn test_encode_webp_lossy() {
    let steps = reencode_with(s::EncoderPreset::WebPLossy{quality:90f32});

    compare_encoded_to_source(IoTestEnum::Url(FRYMIRE_URL.to_owned()),
                              DEBUG_GRAPH,
                              Constraints {
                                  max_file_size: Some(425_000),
                                  similarity: Similarity::AllowDssimMatch(0., 0.01),
                              },
                              steps
    );
}

pub fn reencode_with(preset: s::EncoderPreset) -> Vec<s::Node>{
    vec![
        s::Node::Decode { io_id: 0, commands: None },
        s::Node::Encode {
            io_id: 1,
            preset,
        },
    ]
}
pub fn reencode_with_command(command: &str) -> Vec<s::Node>{
    vec![
        s::Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: command.to_owned(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None
        }
    ]
}

/// Compares the encoded result of a given job to the source. If there is a checksum mismatch, a percentage of off-by-one bytes can be allowed.
/// The output io_id is 1
pub fn compare_encoded_to_source(input: IoTestEnum, debug: bool, require: Constraints, steps: Vec<s::Node>) -> bool {

    let input_copy = input.clone();




    let execute = s::Execute001 {
        graph_recording: default_graph_recording(debug),
        security: None,
        framewise: s::Framewise::Steps(steps)
    };

    if debug {
        println!("{}", serde_json::to_string_pretty(&execute).unwrap());
    }

    let mut context = Context::create().unwrap();
    IoTestTranslator{}.add(&mut context, 0, input).unwrap();
    IoTestTranslator{}.add(&mut context, 1, IoTestEnum::OutputBuffer).unwrap();

    let response = context.execute_1(execute).unwrap();

    match response{

        ResponsePayload::JobResult(r) => {
            assert_eq!(r.decodes.len(), 1);
            assert!(r.decodes[0].preferred_mime_type.len() > 0);
            assert!(r.decodes[0].preferred_extension.len() > 0);
            assert!(r.decodes[0].w > 0);
            assert!(r.decodes[0].h > 0);
            assert_eq!(r.encodes.len(), 1);
            assert!(r.encodes[0].preferred_mime_type.len() > 0);
            assert!(r.encodes[0].preferred_extension.len() > 0);
            assert!(r.encodes[0].w > 0);
            assert!(r.encodes[0].h > 0);
        }
        _ => {}
    }

    let bytes = context.get_output_buffer_slice(1).unwrap();

    let ctx = ChecksumCtx::visuals();

    let mut context2 = Context::create().unwrap();

    let bitmap_key = decode_input(&mut context2, input_copy);
    let original_checksum;
    {
        let bitmaps = context2.borrow_bitmaps()
            .map_err(|e| e.at(here!())).unwrap();

        let mut original = bitmaps.try_borrow_mut(bitmap_key)
            .map_err(|e| e.at(here!())).unwrap();

        let mut original_window = original.get_window_u8().unwrap();


        original_checksum = ChecksumCtx::checksum_bitmap_window(&mut original_window);
        ctx.save_frame(&mut original_window, &original_checksum);
    }

    compare_with(&ctx, &original_checksum, &context2, bitmap_key, ResultKind::Bytes(bytes), require, true)

}
