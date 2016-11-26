use std::fs::File;
use std::io::Read;
extern crate clap;
extern crate imageflow_types as s;
extern crate imageflow_core as fc;
extern crate serde_json;
mod self_test;

use clap::{App, Arg, SubCommand};

fn main() {
    let version = s::version::one_line_version();
    let matches = App::new("imageflow_tool").version(version.as_ref())
        .subcommand(
            SubCommand::with_name("diagnose")
                .about("Diagnostic utilities")
                .arg(
                    Arg::with_name("show-compilation-info").long("show-compilation-info")
                        .help("Show all the information stored in this executable about the environment in which it was compiled.")
                ).arg(
                Arg::with_name("self-test").long("self-test")
                    .help("Creates a 'self_tests' directory and runs self-tests")
            )
        )

        // --json [path]
        // --response [response_json_path]
        // --demo [name]
        // --in 0 a.png b.png
        // --out a.png
        // --local-only (prevent remote URL requests)
        // --no-io-ids (Disables interpretation of numbers in --in and --out as io_id assignment).
        // --no-clobber
        // --debug (verbose, graph export, frame export?)
        // --debug-package




        // file.json --in a.png a.png --out s.png
        // file.json --in 0 a.png 1 b.png --out 3 base64


        .subcommand(SubCommand::with_name("v0.1/build")
            .about("Runs the given operation file")
            .arg(
                Arg::with_name("in")
                .min_values(1)
                .multiple(true)
                .help("Replace/add inputs for the operation file")
            )
                        .arg(Arg::with_name("out").min_values(1).multiple(true)
                            .help("Replace/add outputs for the operation file"))
            .arg(Arg::with_name("demo").possible_values(&["example:200x200_png"]))
            .arg(Arg::with_name("json").takes_value(true).required_unless("demo"))
                        .arg(Arg::with_name("response").takes_value(true))
        )
        .get_matches();


    if let Some(ref matches) = matches.subcommand_matches("v0.1/build") {
        let m : &&clap::ArgMatches = matches;

        let inputs = m.values_of("in");
        let outputs = m.values_of("out");


        if m.is_present("demo"){

        }else {
            let json_path = m.value_of("json").unwrap();
            let exit_code = build(json_path);

        }


    }

    if let Some(ref matches) = matches.subcommand_matches("diagnose") {
        let m : &&clap::ArgMatches = matches;

        if m.is_present("show-compilation-info"){
            println!("{}\n{}\n", s::version::one_line_version(), s::version::all_build_info_pairs());
        }

        if m.is_present("self-test"){
            self_test::run();
        }
    }

}

fn build(json_file: &str) -> i32{

    //TODO: we gotta filter and restrict
    let mut data = Vec::new();
    let mut f = File::open(json_file).expect("Unable to open file");
    f.read_to_end(&mut data).expect("Unable to read data");


    let mut context = fc::Context::create().unwrap();

    let response: fc::JsonResponse = context.message("v0.1/build", &serde_json::to_vec(&data).unwrap()).unwrap();

    println!("{}",  std::str::from_utf8(response.response_json.as_ref()).unwrap());
    if response.status_2xx() {
        0
    }else{
        response.status_code as i32
    }
}


// imageflow_client is the destination-agnostic one.
// imageflow_tool knows only the library

//Priority
//1. Load JSON file, run, output to STDOUT
//2. Show examples of JSON content
//3. Add input overrides
//4.


// For stdin, we could just wait until we accumulate a single valid JSON object?

// type test.json | imageflow_tool -m"0.0.1/build" --edit inputurl,0,https://,output-path,1,file.ext | imageflow_tool

// imageflow_tool --filter --method --edit
// imageflow_tool -m"v0.1/build"
// imageflow_tool --method "v0.1/build" --json path|stdin|named --edit input-url 0 https://etc --edit input-path 1 image.png --edit output-path 2 image.{{ext}}
// imageflow_tool -m"0.0.1/build" --set-io input_for_io_id_0.jpg output_for_io_id_1.png

// imageflow_tool build --version=0.0.1
// imageflow_tool v0.1/build path.json
// imageflow_tool v0.1/build --filter-only
// imageflow_tool v0.1/get_image_info --examples





// Should transformed JSON be written to filesystem? I.e, require helper??
// JSON response to stdout



// should JSON file indicate method and version inside? - nope, don't want a god struct. But we could parse twice, once just to check the header... Eliminates a name we have to pass to all tools. Also means we have to edit json to test backwards compatibility.

// preprocessing breaks debugging - we should do a full parse before any transformations
// Should correspond to context commands, not job, ideally.
// JSON transform: insert I/O by io_id
// JSON transform: replace IO strings by io_id?
// JSON transform: set output filenames


//What about for the very first version?

//JSON file
//Named presets
//physical file I/O
//Remote URLs.
//Local cache folder permissible?
//Replacing I/O


//The stateless client is responsible for collecting it (for now - it's easier)

