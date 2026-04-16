use imageflow_core::Context;
use imageflow_types as s;
fn main() {
    let mut ctx = Context::create().unwrap();
    ctx.add_output_buffer(1).unwrap();
    let jpeg = {
        ctx.execute_1(s::Execute001 {
            graph_recording: Some(s::Build001GraphRecording::off()),
            security: None,
            job_options: None,
            framewise: s::Framewise::Steps(vec![
                s::Node::CreateCanvas {
                    w: 64,
                    h: 64,
                    format: s::PixelFormat::Bgra32,
                    color: s::Color::Srgb(s::ColorSrgb::Hex("FF0000FF".into())),
                },
                s::Node::Encode {
                    io_id: 1,
                    preset: s::EncoderPreset::LibjpegTurbo {
                        quality: Some(85),
                        progressive: Some(false),
                        optimize_huffman_coding: None,
                        matte: None,
                    },
                },
            ]),
        })
        .unwrap();
        ctx.take_output_buffer(1).unwrap()
    };
    // Try decode-only with no terminal
    let mut ctx2 = Context::create().unwrap();
    ctx2.add_input_vector(0, jpeg).unwrap();
    match ctx2.execute_1(s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(vec![s::Node::Decode { io_id: 0, commands: None }]),
    }) {
        Ok(_) => eprintln!("OK: Decode-only works as terminal"),
        Err(e) => eprintln!("ERR: {e}"),
    }
}
