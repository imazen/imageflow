//! imageflow_tool — CLI for imageflow image processing.
//!
//! Supports two modes:
//! - `imageflow_tool v2/build --json '...'` — execute a JSON pipeline
//! - `imageflow_tool process --in input.jpg --out output.jpg --w 800` — simple resize

use clap::{Arg, Command};
use imageflow_core::Context;

fn main() {
    let matches = Command::new("imageflow_tool")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Image processing via the imageflow pipeline")
        .subcommand(
            Command::new("v2/build")
                .about("Execute a JSON build request")
                .arg(
                    Arg::new("json")
                        .long("json")
                        .help("JSON build request")
                        .required(true),
                ),
        )
        .subcommand(
            Command::new("probe")
                .about("Probe an image for metadata")
                .arg(
                    Arg::new("input")
                        .long("in")
                        .help("Input image file")
                        .required(true),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        Some(("v2/build", sub)) => {
            let json_str = sub.get_one::<String>("json").unwrap();
            let ctx = Context::new();
            let response = ctx.send_json("v2/build", json_str.as_bytes());
            let output = String::from_utf8_lossy(&response.response_json);
            if response.status_code == 200 {
                println!("{output}");
            } else {
                eprintln!("{output}");
                std::process::exit(1);
            }
        }
        Some(("probe", sub)) => {
            let input_path = sub.get_one::<String>("input").unwrap();
            let data = std::fs::read(input_path).unwrap_or_else(|e| {
                eprintln!("failed to read {input_path}: {e}");
                std::process::exit(1);
            });
            let ctx = Context::new();
            ctx.add_input_buffer(0, &data).unwrap_or_else(|e| {
                eprintln!("failed to add input: {e}");
                std::process::exit(1);
            });
            let json = br#"{"io_id":0}"#;
            let response = ctx.send_json("v2/get_image_info", json);
            let output = String::from_utf8_lossy(&response.response_json);
            println!("{output}");
        }
        _ => {
            eprintln!("use --help for usage");
            std::process::exit(1);
        }
    }
}
