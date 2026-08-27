//! Regression tests for issue #622: `imageflow_tool v1/querystring --out -`
//! streams the encoded image to stdout (and `--in -` reads from stdin).

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use imageflow_core::Context;
use imageflow_types as s;

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
// IEND chunk: length 0, "IEND", CRC.
const PNG_IEND: &[u8] = b"\x00\x00\x00\x00IEND\xAE\x42\x60\x82";

fn tool() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_imageflow_tool"))
}

fn scratch_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("test_stdio").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Build a small solid PNG in-process so the test needs no fixtures or network.
fn make_png(w: usize, h: usize) -> Vec<u8> {
    let mut ctx = Context::create().unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(s::Execute001 {
        graph_recording: None,
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::CreateCanvas {
                w,
                h,
                format: s::PixelFormat::Bgra32,
                color: s::Color::Srgb(s::ColorSrgb::Hex("2266AAFF".to_owned())),
            },
            s::Node::Encode { io_id: 1, preset: s::EncoderPreset::libpng32() },
        ]),
    })
    .unwrap();
    let bytes = ctx.take_output_buffer(1).unwrap();
    assert!(bytes.starts_with(PNG_SIGNATURE));
    bytes
}

fn assert_is_exactly_one_png(stdout: &[u8]) {
    assert!(
        stdout.starts_with(PNG_SIGNATURE),
        "stdout should begin with the PNG signature, got {:?}",
        &stdout[..stdout.len().min(16)]
    );
    assert!(
        stdout.ends_with(PNG_IEND),
        "stdout should end with the PNG IEND chunk (no JSON appended), got trailing {:?}",
        String::from_utf8_lossy(&stdout[stdout.len().saturating_sub(64)..])
    );
}

#[test]
fn querystring_out_dash_writes_image_to_stdout_and_suppresses_json() {
    let dir = scratch_dir("out_dash");
    let input = dir.join("in.png");
    std::fs::write(&input, make_png(8, 8)).unwrap();

    let output = Command::new(tool())
        .args(["v1/querystring", "--in"])
        .arg(&input)
        .args(["--out", "-", "--command", "w=4&h=4&format=png"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "exit {:?}; stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_is_exactly_one_png(&output.stdout);
    assert!(
        !output.stdout.windows(7).any(|w| w == b"\"code\":"),
        "JSON response must not be written to stdout when --out - is used"
    );
}

#[test]
fn querystring_out_dash_with_response_file_keeps_json() {
    let dir = scratch_dir("out_dash_response");
    let input = dir.join("in.png");
    std::fs::write(&input, make_png(8, 8)).unwrap();
    let response = dir.join("response.json");

    let output = Command::new(tool())
        .args(["v1/querystring", "--in"])
        .arg(&input)
        .args(["--out", "-", "--response"])
        .arg(&response)
        .args(["--command", "w=4&h=4&format=png"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert_is_exactly_one_png(&output.stdout);

    let json = std::fs::read_to_string(&response).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["success"], serde_json::Value::Bool(true), "response: {json}");
}

#[test]
fn querystring_in_dash_reads_image_from_stdin() {
    let png = make_png(8, 8);

    let mut child = Command::new(tool())
        .args(["v1/querystring", "--in", "-", "--out", "-", "--command", "w=2&h=2&format=png"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&png).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert_is_exactly_one_png(&output.stdout);
    // 2x2 output is smaller than the 8x8 input; a decode of stdin actually happened.
    assert!(output.stdout.len() < png.len());
}

#[test]
fn build_out_dash_works_for_json_recipes_too() {
    let dir = scratch_dir("build_out_dash");
    let input = dir.join("in.png");
    std::fs::write(&input, make_png(8, 8)).unwrap();
    let recipe = dir.join("recipe.json");
    let job = s::Build001 {
        builder_config: None,
        io: vec![s::IoEnum::Placeholder.into_input(0), s::IoEnum::Placeholder.into_output(1)],
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode { io_id: 1, preset: s::EncoderPreset::libpng32() },
        ]),
    };
    std::fs::write(&recipe, serde_json::to_vec(&job).unwrap()).unwrap();

    let output = Command::new(tool())
        .args(["v1/build", "--json"])
        .arg(&recipe)
        .args(["--in"])
        .arg(&input)
        .args(["--out", "-"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert_is_exactly_one_png(&output.stdout);
}

#[test]
fn two_stdout_outputs_are_rejected() {
    let dir = scratch_dir("two_out_dash");
    let input = dir.join("in.png");
    std::fs::write(&input, make_png(8, 8)).unwrap();
    let recipe = dir.join("recipe.json");
    let job = s::Build001 {
        builder_config: None,
        io: vec![
            s::IoEnum::Placeholder.into_input(0),
            s::IoEnum::Placeholder.into_output(1),
            s::IoEnum::Placeholder.into_output(2),
        ],
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode { io_id: 1, preset: s::EncoderPreset::libpng32() },
            s::Node::Encode { io_id: 2, preset: s::EncoderPreset::libpng32() },
        ]),
    };
    std::fs::write(&recipe, serde_json::to_vec(&job).unwrap()).unwrap();

    let output = Command::new(tool())
        .args(["v1/build", "--json"])
        .arg(&recipe)
        .args(["--in"])
        .arg(&input)
        .args(["--out", "-", "-"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success(), "two `--out -` must be rejected");
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Only one output may be written to stdout"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
