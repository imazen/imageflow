use ::Context;
use ::Job;
use ::JsonResponse;

pub use imageflow_types::Framewise;
use ::internal_prelude::works_everywhere::*;

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
}


#[derive(Debug, PartialEq)]
pub enum BuildFailure {
    OutOfMemory,
    TransportFailure,
    Error { httpish_code: i32, message: String },
}

impl From<::FlowError> for BuildFailure {
    fn from(e: ::FlowError) -> BuildFailure {
        match e {
            FlowError::Oom => BuildFailure::OutOfMemory,
            other => {
                BuildFailure::Error {
                    httpish_code: 500,
                    message: format!("{:?}", other),
                }
            }
        }
    }
}
impl BuildFailure {
    fn from_parse_error(http_code: i32,
                        prefix: String,
                        error: serde_json::error::Error,
                        json: &[u8])
                        -> BuildFailure {
        let message = format!("{}: {}\n Parsing {}",
                              prefix,
                              error,
                              std::str::from_utf8(json).unwrap_or("[INVALID UTF-8]"));
        BuildFailure::Error {
            httpish_code: http_code,
            message: message,
        }
    }
}


impl LibClient {
    pub fn new() -> LibClient {
        LibClient {}
    }

    pub fn get_image_info(&mut self,
                              bytes: &[u8])
                              -> std::result::Result<s::ImageInfo, BuildFailure> {
        let context = Context::create()?;

        let result = {
            let mut job = context.create_job();
            job.add_input_bytes(0, bytes)?;
            let info_blob: JsonResponse =
                job.message("v0.1/get_image_info", b"{\"io_id\": 0}")?;
            // TODO: add into error conversion
            let info_response: s::Response001 =
                match serde_json::from_slice(info_blob.response_json.as_ref()){
                    Ok(v) => v,
                    Err(e) =>{
                        panic!("Failed to parse JSON response {:?} {:?}", e, str::from_utf8(info_blob.response_json.as_ref()));
                    }
                };
            if !info_response.success {
                panic!("get_image_info failed: {:?}", info_response);
            }
            match info_response.data {
                s::ResponsePayload::ImageInfo(info) => Ok(info),
                _ => {
                    Err(BuildFailure::Error {
                        httpish_code: 500,
                        message: "Endpoint failed to return imageinfo".to_owned(),
                    })
                }
            }
        };
        // TODO: Catch and report instead of panicing
        context.destroy_allowing_panics();
        result
    }


    pub fn build(&mut self, task: BuildRequest) -> std::result::Result<BuildSuccess, BuildFailure> {
        let context = Context::create()?;

        let result = {
            let mut job = context.create_job();

            for input in task.inputs {
                job.add_input_bytes(input.io_id, input.bytes)?;
            }

            // Assume output ids only come from nodes
            for node in task.framewise.clone_nodes() {
                if let s::Node::Encode { ref io_id, .. } = *node {
                    job.add_output_buffer(*io_id)?;
                }
                if let s::Node::CommandString { ref encode, ..} = *node{
                    if let &Some(io_id) = encode{
                        job.add_output_buffer(io_id)?;
                    }
                }
            }

            let send_execute = s::Execute001 {
                framewise: task.framewise,
                graph_recording: match task.export_graphs_to {
                    Some(_) => Some(s::Build001GraphRecording::debug_defaults()),
                    None => None,
                },
                no_gamma_correction: None,
            };


            let send_execute_str = serde_json::to_string_pretty(&send_execute).unwrap();
            let result_blob: JsonResponse =
                job.message("v0.1/execute", send_execute_str.as_bytes())?;

            let result: s::Response001 =
                match serde_json::from_slice(result_blob.response_json.as_ref()) {
                    Ok(r) => r,
                    Err(e) => {
                        return Err(BuildFailure::from_parse_error(500, "Error parsing libimageflow response".to_owned(), e, result_blob.response_json.as_ref()));
                    }
                };

            if !result.success {
                return Err(BuildFailure::Error {
                    httpish_code: 500,
                    message: format!("v0.1/execute failed: {:?}", result),
                });
            }

            let encodes: Vec<s::EncodeResult> = match result.data {
                s::ResponsePayload::JobResult(s::JobResult { encodes }) => encodes,
                _ => {
                    return Err(BuildFailure::Error {
                        httpish_code: 500,
                        message: "Endpoint failed to return JobResult".to_owned(),
                    })
                }
            };

            let mut outputs = Vec::new();
            for encode in encodes {
                outputs.push(BuildOutput {
                    bytes: job.io_get_output_buffer_copy(encode.io_id)?,
                    io_id: encode.io_id,
                    mime_type: encode.preferred_mime_type,
                    file_ext: encode.preferred_extension,
                    w: Some(encode.w as u32),
                    h: Some(encode.h as u32),
                });
            }

            Ok(BuildSuccess { outputs: outputs })
        };
        // TODO: Catch and report instead of panicing
        context.destroy_allowing_panics();
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
