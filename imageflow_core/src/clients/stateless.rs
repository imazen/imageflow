//!
//! This module provides a thin wrapper over job building and image info retrieval.
//! It catches panics and reports them as part of a very simple error enum.
//!
//! It would be nice for this to go away or be merged with Context
//!

use crate::Context;
use crate::JsonResponse;
use crate::ErrorCategory;
use crate::errors::PanicFormatter;

pub use imageflow_types::Framewise;
use crate::internal_prelude::works_everywhere::*;

#[derive(Default)]
pub struct LibClient {

}

#[derive(Clone, PartialEq, Debug)]
pub struct BuildInput<'a> {
    pub io_id: i32,
    pub bytes: &'a [u8],
}

#[derive(Clone, PartialEq, Debug)]
pub struct BuildOutput {
    pub io_id: i32,
    pub bytes: Vec<u8>,
    pub mime_type: String,
    pub file_ext: String,
    pub w: Option<u32>,
    pub h: Option<u32>,
}
#[derive(Clone, PartialEq, Debug)]
pub struct BuildRequest<'a> {
    pub inputs: Vec<BuildInput<'a>>,
    pub framewise: s::Framewise,
    pub export_graphs_to: Option<std::path::PathBuf>, /* TODO: Debugging
                                                       * TODO: Benchmarking
                                                       * TODO: gamma correction
                                                       * TODO: Optimization sets */
}

#[derive(Clone, PartialEq, Debug)]
pub struct BuildSuccess {
    pub outputs: Vec<BuildOutput>,
    pub performance: Option<s::BuildPerformance>
}

#[derive(Debug, PartialEq)]
pub enum BuildFailure {
    OutOfMemory,
    TransportFailure,
    Error { httpish_code: i32, message: String },
}

impl From<crate::FlowError> for BuildFailure {
    fn from(e: crate::FlowError) -> BuildFailure {
        match e.category() {
            ErrorCategory::OutOfMemory => BuildFailure::OutOfMemory,
            other => {
                BuildFailure::Error {
                    httpish_code: e.category().http_status_code(),
                    message: format!("{}", e),
                }
            }
        }
    }
}


use std::panic::{catch_unwind, AssertUnwindSafe};

impl LibClient {

    pub fn new() -> LibClient {
        LibClient {}
    }


     fn get_image_info_inner(context: &mut Context, bytes: &[u8])
                          -> std::result::Result<s::ImageInfo, FlowError> {
        context.add_input_bytes(0, bytes).map_err(|e| e.at(here!()))?;
        Ok(context.get_image_info(0).map_err(|e| e.at(here!()))?)

    }
    pub fn get_image_info(&mut self, bytes: &[u8])
                              -> std::result::Result<s::ImageInfo, BuildFailure> {
        let mut context = Context::create().map_err(|e| e.at(here!()))?;

        let result = catch_unwind(AssertUnwindSafe(||{
            LibClient::get_image_info_inner(&mut context, bytes).map_err(|e| BuildFailure::from(e.at(here!())))
        }));

        let result = match result{
            Err(panic) => Err(BuildFailure::Error{ httpish_code: 500, message: format!("{}", PanicFormatter(&panic))}),
            Ok(Err(e)) => Err(BuildFailure::from(e)),
            Ok(Ok(v)) => Ok(v)
        };

        context.destroy()?; // Termination errors trump execution errors/panics
        result

    }

    fn build_inner(context: &mut Context, task: BuildRequest) -> std::result::Result<BuildSuccess, FlowError> {

        for input in task.inputs {
            context.add_input_buffer(input.io_id, input.bytes).map_err(|e| e.at(here!()))?;
        }

        // Assume output ids only come from nodes
        for node in task.framewise.clone_nodes() {
            if let s::Node::Encode { ref io_id, .. } = *node {
                context.add_output_buffer(*io_id).map_err(|e| e.at(here!()))?;
            }
            if let s::Node::CommandString { ref encode, ..} = *node{
                if let Some(io_id) = *encode{
                    context.add_output_buffer(io_id).map_err(|e| e.at(here!()))?;
                }
            }
        }

        let send_execute = s::Execute001 {
            framewise: task.framewise,
            security: None,
            graph_recording: match task.export_graphs_to {
                Some(_) => Some(s::Build001GraphRecording::debug_defaults()),
                None => None,
            }
        };

        let payload = context.execute_1(send_execute).map_err(|e| e.at(here!()))?;


        let (encodes, perf): (Vec<s::EncodeResult>, Option<s::BuildPerformance>) = match payload {
            s::ResponsePayload::JobResult(s::JobResult { encodes, performance}) => (encodes, performance),
            _ => {
                unreachable!()
            }
        };

        let mut outputs = Vec::new();
        for encode in encodes {
            outputs.push(BuildOutput {
                bytes: context.get_output_buffer_slice(encode.io_id).map(|s|  s.to_vec()).map_err(|e| e.at(here!()))?,
                io_id: encode.io_id,
                mime_type: encode.preferred_mime_type,
                file_ext: encode.preferred_extension,
                w: Some(encode.w as u32),
                h: Some(encode.h as u32),
            });
        }

        Ok(BuildSuccess { outputs: outputs, performance: perf})
    }
    pub fn build(&mut self, task: BuildRequest) -> std::result::Result<BuildSuccess, BuildFailure> {
        let mut context = Context::create().map_err(|e| e.at(here!()))?;

        let result = catch_unwind(AssertUnwindSafe(||{
            LibClient::build_inner(&mut context, task).map_err(|e| BuildFailure::from(e.at(here!())))
        }));

        let result = match result{
            Err(panic) => Err(BuildFailure::Error{ httpish_code: 500, message: format!("{}", PanicFormatter(&panic))}),
            Ok(Err(e)) => Err(BuildFailure::from(e)),
            Ok(Ok(v)) => Ok(v)
        };

        context.destroy()?; //Termination errors trump execution errors
        result
    }
}


#[test]
fn test_stateless() {
    // Must stay alive for duration
    let png_bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
                         0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
                         0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00,
                         0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
                         0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
                         0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82];
    let req = BuildRequest {
        inputs: vec![BuildInput {
                         io_id: 0,
                         bytes: &png_bytes,
                     }],
        export_graphs_to: None,
        framewise: Framewise::Steps(vec![s::Node::Decode {
                                             io_id: 0,
                                             commands: None,
                                         },
                                         s::Node::Encode {
                                             io_id: 1,
                                             preset: s::EncoderPreset::libpng32(),
                                         }]),
    };
    let result = LibClient {}.build(req).unwrap();
    assert!(result.outputs.len() == 1);
}
