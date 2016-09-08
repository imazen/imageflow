mod nodes;
mod parse_graph;

use std;

extern crate rustc_serialize;
extern crate libc;
use libc::c_void;

use parsing::rustc_serialize::hex::FromHex;

use std::ptr;

use ContextPtr;
use JsonResponse;
use std::collections::HashMap;
extern crate imageflow_serde as s;
extern crate serde;
extern crate serde_json;

use std::error;

use ffi;

use self::parse_graph::GraphTranslator;

use ::Context;
pub struct BuildRequestHandler {

}

//#[test]
//fn leak_mem2() {
//
//    let mut v = Vec::with_capacity(333);
//    v.push(0u8);
//    std::mem::forget(v)
//}

#[derive(Debug)]
pub enum JsonResponseError{
    Oom(()),
    NotImplemented(()),
    Other(Box<std::error::Error>)
}

impl BuildRequestHandler{
    pub fn new () -> BuildRequestHandler{
        BuildRequestHandler{}
    }



    fn steps_to_graph(steps: Vec<s::Node>) -> s::Graph {
        let mut nodes = HashMap::new();
        let mut edges = vec![];
        for (i, item) in steps.into_iter().enumerate() {
            nodes.insert(i.to_string(), item);
            edges.push(s::Edge{
                from: i as i32,
                to: i as i32 + 1,
                kind: s::EdgeKind::Input
            });
        }
        let _ = edges.pop();
        //TODO: implement
       s:: Graph {
           nodes: nodes,
           edges: edges
        }
    }

    pub fn do_and_respond<'a, 'b, 'c, 'd>(&'a self, ctx: &'d mut ContextPtr, json: &'b [u8])  -> Result<JsonResponse<'c>, JsonResponseError> {

        let parsed : s::Build001 = serde_json::from_slice(json).unwrap();
        let cfg = parsed.builder_config;
        let io_vec = parsed.io;
        let graph = match parsed.framewise{
            s::Framewise::Graph(g) => g,
            s::Framewise::Steps(s) => BuildRequestHandler::steps_to_graph(s)
        };

        unsafe {
            let p = ctx.ptr.unwrap();

            //create nodes, develop a map of desired vs. actual node ids.

            let mut g = GraphTranslator::new(p).translate_graph(graph);

            let job = ::ffi::flow_job_create(p);


            /*
                pub io_id: i32,
    pub direction: IoDirection,
    pub io: IoEnum,
    pub checksum: Option<IoChecksum>
            */
            let mut io_list = Vec::new();
            for io_obj in io_vec {
                let mut io_ptr: *mut ffi::JobIO = match io_obj.io {
                    s::IoEnum::BytesHex(hex_string) => {
                        let bytes = hex_string.as_str().from_hex().unwrap();


                        //Leak it until we figure out a better way
                        let buf : *mut u8 = ::ffi::flow_context_calloc(p, 1, bytes.len(), ptr::null(), p as *const libc::c_void, ptr::null(), 0) as *mut u8 ;
                        if buf.is_null(){
                            panic!("OOM");
                        }
                        ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len());

                        let mut io_ptr = ::ffi::flow_io_create_from_memory(p, ::ffi::IoMode::read_seekable,buf, bytes.len(), p as *const libc::c_void, ptr::null());

                        if io_ptr.is_null() {
                            panic!("Failed to create I/O");
                        }
                        io_ptr
                    },
                    s::IoEnum::Filename(path) => {
                        ptr::null()
                    },
                    s::IoEnum::Url(url) => {
                        ptr::null()
                    },
                    s::IoEnum::OutputBuffer => {
                        let io_ptr = ::ffi::flow_io_create_for_output_buffer(p, p as *const libc::c_void);
                        if io_ptr.is_null() {
                            panic!("Failed to create I/O");
                        }
                        io_ptr
                    }
                } as *mut ffi::JobIO;

                let new_direction = match io_obj.direction {
                    s::IoDirection::Input => ffi::IoDirection::In,
                    s::IoDirection::Output => ffi::IoDirection::Out
                };

                io_list.push((io_ptr, io_obj.io_id, new_direction));
            }

            for io_list in io_list {
                if !::ffi::flow_job_add_io(p, job, io_list.0, io_list.1, io_list.2) {
                    panic!("flow_job_add_io failed");
                }
            }


            if !::ffi::flow_job_execute(p, job, &mut g) {
                ctx.assert_ok(Some(g));
            }
            //Create job
            //Add i/o

            //Build

            //TODO: Question, should JSON endpoints populate the Context error stacktrace when something goes wrong? Or populate both (except for OOM).

            //        ::ffi::flow_node_create_canvas
            //
            //        ::ffi::flow_context_
            Err(JsonResponseError::NotImplemented(()))
        }
    }



}

#[test]
fn test_handler(){

    let input_io = s::IoObject {
        io_id: 0,
        direction: s::IoDirection::Input,
        checksum: None,
        io: s::IoEnum::BytesHex("FFD8FFE000104A46494600010101004800480000FFDB004300FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC2000B080001000101011100FFC40014100100000000000000000000000000000000FFDA0008010100013F10".to_owned())
    };

    let output_io = s::IoObject {
        io_id: 1,
        direction: s::IoDirection::Output,
        checksum: None,
        io: s::IoEnum::OutputBuffer
    };

    let mut steps = vec![];
    steps.push(s::Node::Decode {io_id: 0});
    steps.push(s::Node::FlipV);
    steps.push(s::Node::Encode {io_id: 1, encoder: None, encoder_id: None, hints: None});

    let build = s::Build001{
        builder_config: None,
        io: vec![input_io, output_io],
        framewise: s::Framewise::Steps(steps)
    };

    let json_str = serde_json::to_string_pretty(&build).unwrap();

    let handler = BuildRequestHandler::new();

    let mut context = Context::create();

    let mut ctx_cell = context.unsafe_borrow_mut_context_pointer();

    let response = handler.do_and_respond(&mut *ctx_cell, json_str.into_bytes().as_slice());



}
