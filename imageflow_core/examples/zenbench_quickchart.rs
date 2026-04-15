//! Convert a zenbench JSON dump into QuickChart image URLs.
//!
//! Usage:
//!   cargo run --release --example zenbench_quickchart -- <results.json>
//!
//! Reads the SuiteResult JSON produced by `cargo bench ... -- --format=json`
//! and prints one markdown image link per benchmark group.

use std::env;
use std::fs;
use std::process::ExitCode;

use zenbench::quickchart::QuickChartConfig;
use zenbench::SuiteResult;

fn main() -> ExitCode {
    let path = match env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: zenbench_quickchart <results.json>");
            return ExitCode::from(2);
        }
    };
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("read {path}: {e}");
            return ExitCode::from(2);
        }
    };
    let result: SuiteResult = match serde_json::from_str(&text) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("parse {path}: {e}");
            return ExitCode::from(2);
        }
    };
    print!("{}", result.to_quickchart_markdown(&QuickChartConfig::default()));
    ExitCode::SUCCESS
}
