//! Decode a JPEG, immediately re-encode as lossless PNG. No resize, no transform.
//! Run twice (zen and c) and diff the PNGs to measure raw JPEG decoder divergence.

use imageflow_core::Context;
use imageflow_types as s;

fn main() {
    let label = std::env::args().nth(1).expect("usage: <label> <jpeg_path>");
    let path = std::env::args().nth(2).expect("usage: <label> <jpeg_path>");
    let input = std::fs::read(&path).expect("read jpeg");
    eprintln!("{}: decoding {} ({} bytes)", label, path, input.len());

    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(s::Execute001 {
        job_options: None,
        graph_recording: None,
        security: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Lodepng { maximum_deflate: None },
            },
        ]),
    })
    .unwrap();
    let png = ctx.take_output_buffer(1).unwrap();
    let out_path = format!("/tmp/jpeg-decode-{}.png", label);
    std::fs::write(&out_path, &png).unwrap();
    eprintln!("{}: wrote {}", label, out_path);
}
