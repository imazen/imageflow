

use ::{Job, Context};
use ::internal_prelude::works_everywhere::*;
use ::json::*;
use ::parsing::parse_graph::GraphTranslator;
use ::parsing::parse_io::IoTranslator;
use std::error;


fn create_job_router() -> MethodRouter<'static, Job> {
    let mut r = MethodRouter::new();
    r.add_responder("v0.1/get_image_info",
                    Box::new(move |job: &mut Job, data: s::GetImageInfo001| {
                        Ok(s::ResponsePayload::ImageInfo(job.get_image_info(data.io_id)?))
                    }));
    r.add_responder("v0.1/tell_decoder",
                    Box::new(move |job: &mut Job, data: s::TellDecoder001| {
                        job.tell_decoder(data.io_id, data.command)?;
                        Ok(s::ResponsePayload::None)
                    }));
    r.add_responder("v0.1/execute",
                    Box::new(move |job: &mut Job, parsed: s::Execute001| {
        let mut g = ::parsing::GraphTranslator::new().translate_framewise(parsed.framewise)?;
        if let Some(r) = parsed.graph_recording {
            job.configure_graph_recording(r);
        }

        if let Some(b) = parsed.no_gamma_correction {
            job.context().todo_remove_set_floatspace(if b {
                ::ffi::Floatspace::srgb
            }else {
                ::ffi::Floatspace::linear
            });
        };
        //Cheat on lifetimes so Job can remain mutable
        let split_context = unsafe{ &*(job.context() as *const Context)};
        ::flow::execution_engine::Engine::create(split_context, job, &mut g).execute()?;

        Ok(s::ResponsePayload::JobResult(s::JobResult { encodes: Job::collect_encode_results(&g) }))
    }));
    r.add("brew_coffee",
          Box::new(move |job: &mut Job, bytes: &[u8]| Ok(JsonResponse::teapot())));
    r
}

lazy_static! {
        pub static ref JOB_ROUTER: MethodRouter<'static, Job> = create_job_router();
    }


fn document_message() -> String {
    let mut s = String::new();
    s.reserve(8000);
    s += "JSON API - Job\n\n";
    s += "imageflow_job responds to these message methods\n\n";
    s += "## v0.1/get_image_info \n";
    s += "Example message body:\n";
    s += &serde_json::to_string_pretty(&s::GetImageInfo001::example_get_image_info()).unwrap();
    s += "\nExample response:\n";
    s += &serde_json::to_string_pretty(&s::Response001::example_image_info()).unwrap();
    s += "\n\n";


    s += "## v0.1/tell_decoder \n";
    s += "Example message body:\n";
    s += &serde_json::to_string_pretty(&s::TellDecoder001::example_hints()).unwrap();
    s += "\nExample response:\n";
    s += &serde_json::to_string_pretty(&s::Response001::example_ok()).unwrap();
    s += "\n\n";

    s += "## v0.1/execute \n";
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


// env!(CARGO_PKG_NAME)
// env!(CARGO_PKG_HOMEPAGE)
// env!(CARGO_MANIFEST_DIR)

fn get_create_doc_dir() -> std::path::PathBuf {
    let path = ::imageflow_types::version::crate_parent_folder().join(Path::new("target/doc"));
    let _ = std::fs::create_dir_all(&path);
    // Error { repr: Os { code: 17, message: "File exists" } }
    // The above can happen, despite the docs.
    path
}


#[test]
fn write_job_doc() {
    let path = get_create_doc_dir().join(Path::new("job_json_api.txt"));
    File::create(&path).unwrap().write_all(document_message().as_bytes()).unwrap();
}
