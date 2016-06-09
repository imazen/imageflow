extern crate clap;
use clap::*;
extern crate imageflow_server;
use imageflow_server::boring::*;

fn main() {
    let matches = App::new("flow-proto1")
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
        .arg(Arg::with_name("incorrectgamma").long("incorrectgamma")
             .help("Enables incorrect gamma handling (for benchmarking comparison purposes)."))
        .arg(Arg::with_name("min_precise_scaling_ratio")
            .short("mpsr")
            .value_name("MINRATIO")
            .takes_value(true)
            .help("Defaults to 2.1. Jpeg-integrated block scaling is permitted down to 2.1x \
                   final size"))
        .get_matches();


    // let verbosity = matches.occurrences_of("v");
    // Vary the output based on how many times the user used the "verbose" flag
    // (i.e. 'myprog -v -v -v' or 'myprog -vvv' vs 'myprog -v'

    let w = matches.value_of("width").unwrap_or("0").parse().unwrap();
    let h = matches.value_of("height").unwrap_or("0").parse().unwrap();
    let in_file = matches.value_of("input").unwrap();
    let out_file = matches.value_of("output").unwrap();

    let min_precise_scaling_ratio =
        matches.value_of("min_precise_scaling_ratio").unwrap_or("3").parse().unwrap();

    proccess_image(in_file,
                   out_file,
                   BoringCommands {
                       w: w,
                       h: h,
                       fit: ConstraintMode::Max,
                       precise_scaling_ratio: min_precise_scaling_ratio,
                       luma_correct: !matches.is_present("incorrectgamma"),
                   });


}
