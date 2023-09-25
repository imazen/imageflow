
extern crate clap;
extern crate imageflow_helpers;
extern crate imageflow_types as s;
extern crate imageflow_core as fc;
extern crate serde_json;

use std::ffi::OsStr;
use imageflow_helpers as hlp;

use std::path::{Path,PathBuf};
mod cmd_build;
pub mod self_test;


use clap::{Arg, Command, ValueHint, ArgAction};


fn artifact_source() -> hlp::process_capture::IncludeBinary{
    hlp::process_capture::IncludeBinary::UrlOrCopy(s::version::get_build_env_value("ESTIMATED_ARTIFACT_URL").map(|v| v.to_owned()))
}

pub fn main_with_exit_code() -> i32 {
    imageflow_helpers::debug::set_panic_hook_once();
    let str: &'static str = Box::leak(s::version::one_line_version().into_boxed_str());
    let app = Command::new("imageflow_tool").version(str)
        .arg(  Arg::new("capture-to").long("capture-to").num_args(1).value_parser(clap::value_parser!(PathBuf)).global(true)
            .help("Run whatever you're doing in a sub-process, capturing output, input, and version detail")
        ).subcommand_required(true).arg_required_else_help(true)
        .subcommand(
            Command::new("diagnose").arg_required_else_help(true)
                .about("Diagnostic utilities")
                .arg(
                    Arg::new("show-compilation-info").long("show-compilation-info").num_args(0)
                        .action(ArgAction::SetTrue)
                        .help("Show all the information stored in this executable about the environment in which it was compiled.")
                ).arg(
                Arg::new("self-test").long("self-test").num_args(0)
                    .action(ArgAction::SetTrue)
                    .help("Creates a 'self_tests' directory and runs self-tests"))
                .arg(
                    Arg::new("wait").long("wait").num_args(0)
                        .action(ArgAction::SetTrue)
                        .help("Process stays in memory until you press the enter key.")
                )
                .arg(
                    Arg::new("call-panic").long("call-panic").num_args(0)
                        .action(ArgAction::SetTrue)
                        .help("Triggers a Rust panic (so you can observe failure/backtrace behavior)")
                )
        )
        .subcommand(
            Command::new("examples")
                .about("Generate usage examples")
                .arg(
                    Arg::new("generate").long("generate").required(true).num_args(0)
                        .action(ArgAction::SetTrue)
                        .help("Create an 'examples' directory")
                )
        )

        // --json [path]
        // --response [response_json_path]
        // --demo [name]
        // --in 0 a.png b.png
        // --out a.png

        //Eventually:
        // --local-only (prevent remote URL requests)
        // --no-io-ids (Disables interpretation of numbers in --in and --out as io_id assignment).
        // --no-clobber
        // --debug (verbose, graph export, frame export?)
        // --debug-package




        // file.json --in a.png a.png --out s.png
        // file.json --in 0 a.png 1 b.png --out 3 base64


        .subcommand(Command::new("v1/build").alias("v0.1/build")
            .about("Runs the given operation file")
            .arg(
                Arg::new("in").long("in").num_args(1..)
                    .action(clap::ArgAction::Append)
                    .value_hint(ValueHint::FilePath)
                    // Since the s::Build01 requires valid UTF8, it's better to reject it early.
                    //.value_parser(clap::value_parser!(PathBuf))
                    .value_names(["source-image.jpg", "source-image-2.png"])
                    .help("Replace/add inputs for the operation file")
            )
            .arg(Arg::new("out").long("out").action(clap::ArgAction::Append).num_args(1..)
                // Since the s::Build01 requires valid UTF8, it's better to reject it early.
                //.value_parser(clap::value_parser!(PathBuf))
                .value_hint(ValueHint::FilePath)
                .value_names(["result-1.jpg"])
                .help("Replace/add outputs for the operation file"))
            //.arg(Arg::new("demo").long("demo").takes_value(true).possible_values(&["example:200x200_png"]))
            .arg(Arg::new("json").long("json").num_args(1)
                .value_hint(ValueHint::FilePath)
                .value_parser(clap::value_parser!(PathBuf))
                .value_names(["job.json"])
                .required(true).help("The JSON operation file."))
            .arg(Arg::new("quiet").long("quiet").num_args(0).action(ArgAction::SetTrue).help("Don't write the JSON response to stdout"))
            .arg(Arg::new("response").long("response").num_args(1).value_hint(ValueHint::FilePath).value_parser(clap::value_parser!(PathBuf)).help("Write the JSON job result to file instead of stdout"))
            .arg(Arg::new("bundle-to").long("bundle-to").num_args(1).value_hint(ValueHint::DirPath).value_parser(clap::value_parser!(PathBuf)).help("Copies the recipe and all dependencies into the given folder, simplifying it."))
            .arg(Arg::new("debug-package").long("debug-package").num_args(1).value_hint(ValueHint::FilePath).value_parser(clap::value_parser!(PathBuf)).help("Creates a debug package in the given folder so others can reproduce the behavior you are seeing"))

        )
        .subcommand(Command::new("v1/querystring").aliases(&["v0.1/ir4","v1/ir4"])
            .about("Run an command querystring")
            .arg(
                Arg::new("in").long("in").num_args(1..)
                    // Since the s::Build01 requires valid UTF8, it's better to reject it early.
                    //.value_parser(clap::value_parser!(PathBuf))
                    .action(clap::ArgAction::Append).required(true)
                    .value_hint(ValueHint::FilePath)
                    .value_names(["source-image.jpg", "source-image-2.png"])
                    .help("Input image")
            )
            .arg(Arg::new("out").long("out")
                .action(ArgAction::Append).num_args(1..).required(true)
                //.value_parser(clap::value_parser!(PathBuf))
                .value_hint(ValueHint::FilePath)
                .value_names(["result-1.jpg"])
                .help("Output image"))
            .arg(Arg::new("quiet").action(ArgAction::SetTrue).long("quiet").num_args(0).help("Don't write the JSON response to stdout"))
            .arg(Arg::new("response")
                .long("response")
                .num_args(1)
                .value_names(["result-response.json"])
                .value_hint(ValueHint::FilePath)
                .value_parser(clap::value_parser!(PathBuf))
                .help("Write the JSON job result to file instead of stdout"))
            .arg(Arg::new("command").long("command").num_args(1).required(true)
                .value_names(["w=200&h=200&mode=crop"])
               .help("w=200&h=200&mode=crop&format=png&rotate=90&flip=v - querystring style command"))
            .arg(Arg::new("bundle-to").long("bundle-to").num_args(1).value_hint(ValueHint::DirPath).value_parser(clap::value_parser!(PathBuf)).help("Copies the recipe and all dependencies into the given folder, simplifying it."))
            .arg(Arg::new("debug-package").long("debug-package").num_args(1).value_hint(ValueHint::DirPath).value_parser(clap::value_parser!(PathBuf)).help("Creates a debug package in the given folder so others can reproduce the behavior you are seeing"))

        );
    let matches = app.get_matches();
    if let Some(capture_dest) = matches.get_one("capture-to"){
        let mut filtered_args = std::env::args().collect::<Vec<String>>();
        for ix in 0..filtered_args.len() {
            if filtered_args[ix] == "--capture-to"{
                //Remove this and the next arg
                filtered_args.remove(ix);
                if ix < filtered_args.len() - 1{
                    filtered_args.remove(ix);
                }
                break;
            }
        }
        filtered_args.remove(0); //Remove the tool executable itself

        let cap = hlp::process_capture::CaptureTo::create(capture_dest, None, filtered_args, artifact_source());
        cap.run();
        return cap.exit_code();
    }

    let build_triple = if let Some(m) = matches.subcommand_matches("v1/build") {
        // let source = if m.contains_id("demo") {
        //     cmd_build::JobSource::NamedDemo(m.value_of("demo").unwrap().to_owned())
        // } else {
        let source = cmd_build::JobSource::JsonFile(m.get_one::<PathBuf>("json").unwrap().to_owned());
        //};
        Some((m, source, "v1/build"))
    }else if let Some(m) = matches.subcommand_matches("v1/querystring"){
        Some((m,cmd_build::JobSource::Ir4QueryString(m.get_one::<String>("command").unwrap().to_owned()), "v1/querystring"))
    }else{ None };

    if let Some((m, source, subcommand_name)) = build_triple{

        let builder =
            cmd_build::CmdBuild::parse(source, m.get_many::<String>("in").map(|v| v.cloned().collect()),
                                                m.get_many::<String>("out").map(|v| v.cloned().collect()))
                .build_maybe();
        if let Some(dir_str) = m.get_one::<PathBuf>("debug-package"){
            builder.write_errors_maybe().unwrap();
            let dir = Path::new(&dir_str);
            builder.bundle_to(dir);
            let curdir = std::env::current_dir().unwrap();
            std::env::set_current_dir(&dir).unwrap();
            let cap = hlp::process_capture::CaptureTo::create(&OsStr::new("recipe").to_owned().into(), None, vec![subcommand_name.to_owned(), "--json".to_owned(), "recipe.json".to_owned()], artifact_source());
            cap.run();
            //Restore current directory
            std::env::set_current_dir(&curdir).unwrap();
            let mut archive_name = dir_str.as_os_str().to_owned();
            archive_name.push(".zip");
            hlp::filesystem::zip_directory_nonrecursive(&dir,&Path::new(&archive_name)).unwrap();
            return cap.exit_code();
        } else if let Some(dir) = m.get_one::<PathBuf>("bundle-to").map(|v| v.to_owned()) {
                builder.write_errors_maybe().unwrap();
                let dir = Path::new(&dir);
                return builder.bundle_to(dir);
        } else {
            builder.write_response_maybe(m.get_one("response"), !m.get_flag("quiet"))
                .expect("IO error writing JSON output file. Does the directory exist?");
            builder.write_errors_maybe().expect("Writing to stderr failed!");
            return builder.get_exit_code().unwrap();
        }
    }

    if let Some(matches) = matches.subcommand_matches("diagnose") {
        let m: &clap::ArgMatches = matches;

        if m.get_flag("show-compilation-info") {
            println!("{}\n{}\n",
                     s::version::one_line_version(),
                     s::version::all_build_info_pairs());

            return 0;
        }

        if m.get_flag("self-test") {
            return self_test::run(None);
        }
        if m.get_flag("wait") {
            let mut input_buf = String::new();
            let input = std::io::stdin().read_line(&mut input_buf).expect("Failed to read from stdin. Are you using --wait in a non-interactive shell?");
            println!("{}", input);
            return 0;
        }
        if m.get_flag("call-panic") {
            panic!("Panicking on command");
        }
    }
    if let Some(matches) = matches.subcommand_matches("examples") {
        let m: &clap::ArgMatches = matches;

        if m.get_flag("generate") {
            self_test::export_examples(None);
            return 0;
        }
    }

    64
}

#[test]
fn test_file_macro_for_this_build(){
    assert!(file!().starts_with("imageflow_tool"))
}
