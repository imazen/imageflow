extern crate clap;
use clap::{App, Arg, ArgMatches};
extern crate imageflow_server;
use imageflow_server::boring::*;
use std::path::{PathBuf, Path};
use std::fs::File;


// TODO
// Disclaim use for jpeg optimization
// Disclaim use for png or gif files
// Focus on scaling
// No metadata support
// Let users adjust quality setting

fn build_app() -> App<'static, 'static> {
    App::new("flow-proto1")
        .version("0.0.1")
        .author("Nathanael Jones <imageflow@imazen.io>")
        .about("Prototype tool to play with one feature of libimageflow")
        .arg(Arg::with_name("v")
            .short("v")
            .multiple(true)
            .help("Sets the level of verbosity"))
        .arg(Arg::with_name("input")
            .short("i")
            .value_name("FILEIN")
            .takes_value(true)
            .required(true)
            .help("path to input file"))
        .arg(Arg::with_name("output")
            .short("o")
            .value_name("FILEOUT")
            .takes_value(true)
            .required(true)
            .help("path to output file"))
        .arg(Arg::with_name("width")
            .short("w")
            .value_name("WIDTH")
            .takes_value(true)
            .help("scale to this width or smaller."))
        .arg(Arg::with_name("height")
            .short("h")
            .value_name("HEIGHT")
            .takes_value(true)
            .help("scale to this height or smaller."))
        .arg(Arg::with_name("incorrectgamma")
            .long("incorrectgamma")
            .help("Enables incorrect gamma handling (for benchmarking comparison purposes)."))
        .arg(Arg::with_name("min_precise_scaling_ratio")
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
            fit: ConstraintMode::Max,
            precise_scaling_ratio: min_precise_scaling_ratio.unwrap_or(2.1f32),
            luma_correct: !matches.is_present("incorrectgamma"),
        },
    })

}

fn main() {
    let matches = build_app().get_matches();

    let result = match parse(matches) {
        Ok(c) => proccess_image(c.input_file, c.output_file, c.commands),
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



    let valid_args = vec!["flow-proto1", "-i", "delete.jpg", "-o", "b.jpg", "-w", "20"];

    parse(build_app().get_matches_from(valid_args)).expect("To parse correctly");


    std::fs::remove_file("delete.jpg").unwrap();


}
