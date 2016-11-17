use std;
extern crate imageflow_serde as s;
extern crate serde_json;
use ::ContextPtr;
use ::SelfDisposingContextPtr;
use ::JsonResponse;
use ::JobPtr;
use ::FlowError;

pub use s::Framewise;


pub struct LibClient{

}

#[derive(Clone, PartialEq, Debug)]
pub struct BuildInput<'a>{
    pub io_id: i32,
    pub bytes: &'a [u8]
}

#[derive(Clone, PartialEq, Debug)]
pub struct BuildOutput{
    pub io_id: i32,
    pub bytes: Vec<u8>,
    pub mime_type: &'static str,
    pub file_ext: &'static str,
    pub w: Option<u32>,
    pub h: Option<u32>,
}
#[derive(Clone, PartialEq, Debug)]
pub struct BuildRequest<'a>{
    pub inputs: Vec<BuildInput<'a>>,
    pub output_ids: Vec<i32>,
    pub framewise: s::Framewise,
    //TODO: Debugging
    //TODO: Benchmarking
    //TODO: gamma correction
    //TODO: Optimization sets
}
#[derive(Clone, PartialEq, Debug)]
pub struct BuildSuccess{
    pub outputs: Vec<BuildOutput>
}


#[derive(Debug, PartialEq)]
pub enum BuildFailure{
    OutOfMemory,
    TransportFailure,
    Error{
        httpish_code: i32,
        message: String,
    }
}

impl From<::FlowError> for BuildFailure {
    fn from(e: ::FlowError) -> BuildFailure {
        match e{
            FlowError::Oom => BuildFailure::OutOfMemory,
            other => BuildFailure::Error{httpish_code: 500, message: format!("{:?}", other)}
        }
    }
}


impl LibClient{

    pub fn build(&mut self, task: BuildRequest) -> std::result::Result<BuildSuccess,BuildFailure>  {
        let context = SelfDisposingContextPtr::create()?;

        let result = {
            let mut job: JobPtr = context.create_job()?;

            for input in task.inputs {
                job.add_input_bytes(input.io_id, input.bytes)?;
            }
            for output_id in task.output_ids.iter(){
                job.add_output_buffer(*output_id)?;
            }

            let send_execute = s::Execute001{
                framewise: task.framewise,
                graph_recording: None,
                no_gamma_correction: None
            };

            let send_execute_str = serde_json::to_string_pretty(&send_execute).unwrap();
            job.message("v0.0.1/execute", send_execute_str.as_bytes()).unwrap().assert_ok();

            let mut outputs = Vec::new();
            for io_id in task.output_ids{
                outputs.push(BuildOutput{
                    bytes: job.io_get_output_buffer_copy(io_id)?,
                    io_id: io_id,
                    mime_type: "notimplemented",
                    file_ext: "notimplemented",
                    w: None,
                    h: None
                });
            }

            Ok(BuildSuccess{
                outputs: outputs
            })
        };
        //TODO: Catch and report instead of panicing
        context.destroy_allowing_panics();
        result
    }
}
//
//
//
//#[test]
//fn test_failure(){
//    let req = BuildRequest{
//        output_ids: vec![1],
//        inputs: vec![BuildInput{
//            io_id: 0,
//            bytes: vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
//            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
//            0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
//            0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82 ]
//        }],
//        framewise: Framewise::Steps(vec![
//        s::Node::Decode{io_id: 0},
//        s::Node::Encode{io_id: 1, preset: s::EncoderPreset::libpng32()}
//        ])
//    };
//    let result = LibClient{}.build(req).unwrap();
//    assert!(result.outputs.len() == 1);
//}

#[test]
fn test_stateless(){
    //Must stay alive for duration
    let png_bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
    0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
    0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
    0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82 ];
    let req = BuildRequest{
        output_ids: vec![1],
        inputs: vec![BuildInput{
            io_id: 0,
            bytes: &png_bytes
        }],
        framewise: Framewise::Steps(vec![
        s::Node::Decode{io_id: 0},
        s::Node::Encode{io_id: 1, preset: s::EncoderPreset::libpng32()}
        ])
    };
    let result = LibClient{}.build(req).unwrap();
    assert!(result.outputs.len() == 1);
}