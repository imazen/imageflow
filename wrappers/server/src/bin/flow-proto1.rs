#[macro_use]
extern crate clap;
use clap::{App, Arg, ArgMatches};
extern crate imageflow_server;
use imageflow_server::boring::*;

use imageflow_server::ffi::{Filter, TESTED_FILTER_OPTIONS};
use std::path::{PathBuf, Path};
use std::fs::File;
use std::io::Write;
// TODO
// Disclaim use for jpeg optimization
// Disclaim use for png or gif files
// Focus on scaling
// No metadata support
// Let users adjust quality setting

fn build_app() -> App<'static, 'static> {


    App::new("flow-proto1")
        .version("0.0.1")
        .author("Email us at imageflow@imazen.io")
        .about("Throwaway prototype tool to play with downscaling jpegs via libimageflow. Not for \
                production use. Optimizations disabled on Windows; expect poor perf there. Lots \
                of things are broken, but you should still tell us about them.")
        .arg(Arg::with_name("v")
            .short("v")
            .long("verbose")
            .multiple(true)
            .help("Sets the level of verbosity"))
        .arg(Arg::with_name("input")
            .short("i")
            .long("input")
            .value_name("FILEIN")
            .takes_value(true)
            .required(true)
            .help("path to input file"))
        .arg(Arg::with_name("output")
            .short("o")
            .long("output")
            .value_name("FILEOUT")
            .takes_value(true)
            .required(true)
            .help("path to output file"))
        .arg(Arg::with_name("width")
            .short("w")
            .long("width")
            .value_name("WIDTH")
            .takes_value(true)
            .required(true)
            .help("scale to this width or smaller."))
        .arg(Arg::with_name("height")
            .short("h")
            .long("height")
            .value_name("HEIGHT")
            .takes_value(true)
            .required(true)
            .help("scale to this height or smaller."))
        .arg(Arg::with_name("jpeg-quality")
            .long("jpeg-quality")
            .value_name("0..100")
            .takes_value(true)
            .help("Jpeg compression level."))
        .arg(Arg::with_name("sharpen")
            .long("sharpen")
            .value_name("0..100")
            .takes_value(true)
            .help("Percent sharpening to apply."))
        .arg(Arg::with_name("format")
            .long("format")
            .value_name("png | jpg | png24")
            .takes_value(true)
            .possible_values(&["png", "jpeg", "jpg", "png24"])
            .help("Output image format to use. Baseline jpeg and 32-bit PNG supported."))
        .arg(Arg::with_name("constrain")
            .long("constrain")
            .value_name("max | distort")
            .takes_value(true)
            .possible_values(&["max", "distort"])
            .help("Output image format to use. Baseline jpeg and 32-bit PNG supported."))
        .arg(Arg::with_name("down-filter")
            .long("down-filter")
            .value_name("FILTER")
            .takes_value(true)
            .possible_values(TESTED_FILTER_OPTIONS)
            .help("Filter to use when downscaling"))
        .arg(Arg::with_name("up-filter")
            .long("up-filter")
            .value_name("FILTER")
            .possible_values(TESTED_FILTER_OPTIONS)
            .takes_value(true)
            .help("Filter to use when upscaling"))
        .arg(Arg::with_name("incorrectgamma")
            .long("incorrectgamma")
            .help("Enables incorrect gamma handling (for benchmarking comparison purposes)."))
        .arg(Arg::with_name("min_precise_scaling_ratio")
            .long("min_precise_scaling_ratio")
            .short("mpsr")
            .value_name("MINRATIO")
            .takes_value(true)
            .help("Defaults to 2.1. Jpeg-integrated block scaling is permitted down to 2.1x \
                   final size"))
}

struct ParsedResult {
    input_file: PathBuf,
    output_file: PathBuf,
    commands: BoringCommands,
}


fn parse(matches: ArgMatches) -> Result<ParsedResult, String> {

    let w = matches.value_of("width").and_then(|x| x.parse::<u32>().ok());
    let h = matches.value_of("height").and_then(|x| x.parse::<u32>().ok());

    let sharpen = matches.value_of("sharpen").and_then(|x| x.parse::<f32>().ok()); //.and_then(|x| Some(x / 100f32));


    let q = matches.value_of("jpeg-quality").and_then(|x| x.parse::<i32>().ok());


    let fmt = value_t!(matches, "format", ImageFormat).unwrap_or(ImageFormat::Jpeg);

    let constrain = value_t!(matches, "constrain", ConstraintMode).unwrap_or(ConstraintMode::Max);

    let down_filter = value_t!(matches, "down-filter", Filter).unwrap_or(Filter::Robidoux);
    let up_filter = value_t!(matches, "up-filter", Filter).unwrap_or(Filter::Ginseng);


    // Clap requires these to exist, thus the safe unwrap()
    let in_file = Path::new(matches.value_of("input").unwrap());
    let out_file = Path::new(matches.value_of("output").unwrap());
    let min_precise_scaling_ratio = matches.value_of("min_precise_scaling_ratio")
        .and_then(|x| x.parse::<f32>().ok());


    if w.or(h).is_none() {
        return Err("You must specified at least one of (width, height) ".to_owned());
    }
    if !in_file.is_file() {
        return Err(format!("The specified input file could not be found: {}",
                           in_file.to_str().unwrap()));
    }

    // parent() will return "", an empty path. We don't want to validate the current dir exists, that's going too far.
    let parent_exists = out_file.parent()
        .and_then(|p| match p.to_str().unwrap() {
            "" => None,
            _ => Some(p),
        })
        .map(|p| p.exists());

    // It's OK if there's no parent directory specified.
    // It's only bad if there's one specified and it doesn't exists.
    let warn_about_parent = !parent_exists.unwrap_or(true);
    if warn_about_parent {
        return Err(format!("The parent directory for the output file could not be found: {}",
                           out_file.parent().unwrap().to_str().unwrap()));
    }

    Ok(ParsedResult {
        input_file: in_file.to_path_buf(),
        output_file: out_file.to_path_buf(),
        commands: BoringCommands {
            w: w.unwrap_or(0) as i32,
            h: h.unwrap_or(0) as i32,
            sharpen: sharpen.unwrap_or(0f32) as f32,
            jpeg_quality: q.unwrap_or(90),
            fit: constrain,
            precise_scaling_ratio: min_precise_scaling_ratio.unwrap_or(2.1f32),
            luma_correct: !matches.is_present("incorrectgamma"),
            format: fmt,
            up_filter: up_filter,
            down_filter: down_filter,
        },
    })

}

fn main() {
    let matches = build_app().get_matches();

    let result = match parse(matches) {
        Ok(c) => process_image_by_paths(c.input_file, c.output_file, c.commands),
        Err(e) => Err(e),
    };

    if result.is_err() {
        println!("Failed: {}", result.err().unwrap());
        std::process::exit(1);
    }

}

#[test]
fn test_correct_parsing() {

    File::create("delete.jpg").expect("File creation should work.");



    let valid_args = vec!["flow-proto1",
                          "-i",
                          "delete.jpg",
                          "-o",
                          "b.jpg",
                          "-w",
                          "20",
                          "-h",
                          "20",
                          "--down-filter",
                          "mitchell"];

    let result = parse(build_app().get_matches_from(valid_args)).expect("To parse correctly");

    assert_eq!(result.commands.down_filter, Filter::Mitchell);


    // std::fs::remove_file("delete.jpg").unwrap();


}

#[test]
fn test_correct_execution() {

    {

        let mut f = File::create("test_input.jpg").unwrap();


        let jpeg_bytes =
            &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x01,
              0x00, 0x48, 0x00, 0x48, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0xFF, 0xFF, 0xFF,
              0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
              0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
              0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
              0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
              0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xC2, 0x00, 0x0B, 0x08, 0x00, 0x01, 0x00, 0x01,
              0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00, 0x14, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00,
              0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xDA,
              0x00, 0x08, 0x01, 0x01, 0x00, 0x01, 0x3F, 0x10];


        f.write_all(jpeg_bytes).unwrap();
        f.sync_all().unwrap();
        drop(f);
    }

    let valid_args =
        vec!["flow-proto1", "-i", "test_input.jpg", "-o", "b.jpg", "-w", "20", "-h", "20"];

    let parsed_result = parse(build_app().get_matches_from(valid_args))
        .expect("To parse correctly");

    process_image_by_paths(parsed_result.input_file,
                           parsed_result.output_file,
                           parsed_result.commands)
        .unwrap();

    // std::fs::remove_file("delete.jpg").unwrap();


}
