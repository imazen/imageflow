// Decodes <input> to <output.png> with a chosen CMS backend, for comparing moxcms versions.
// Copy to imageflow_core/examples/moxcms_dump.rs and build it with
// `cargo build -p imageflow_core --example moxcms_dump --profile test --locked`.
use imageflow_core::{CmsBackend, Context};
use imageflow_types as s;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    CmsBackend::set_process_default(match a[1].as_str() {
        "moxcms" => CmsBackend::Moxcms,
        "lcms2" => CmsBackend::Lcms2,
        o => panic!("{o}"),
    });
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, std::fs::read(&a[2]).unwrap()).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(s::Execute001 {
        job_options: None,
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        framewise: s::Framewise::Steps(vec![s::Node::CommandString {
            kind: s::CommandStringKind::ImageResizer4,
            value: "format=png".to_owned(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None,
        }]),
    })
    .unwrap();
    std::fs::write(&a[3], ctx.take_output_buffer(1).unwrap()).unwrap();
}
