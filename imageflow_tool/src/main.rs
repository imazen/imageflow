extern crate clap;
extern crate imageflow_types as s;
extern crate imageflow_core as fc;
extern crate serde_json;
extern crate chrono;
extern crate serde;

mod self_test;
mod cmd_build;

use clap::{App, Arg, SubCommand};

fn main() {
    let exit_code = main_with_exit_code();
    std::process::exit(exit_code);
}
fn main_with_exit_code() -> i32{
    let version = s::version::one_line_version();
    let app = App::new("imageflow_tool").version(version.as_ref())
        .subcommand(
            SubCommand::with_name("diagnose")
                .about("Diagnostic utilities")
                .arg(
                    Arg::with_name("show-compilation-info").long("show-compilation-info")
                        .help("Show all the information stored in this executable about the environment in which it was compiled.")
                ).arg(
                Arg::with_name("self-test").long("self-test")
                    .help("Creates a 'self_tests' directory and runs self-tests"))
                    .arg(
                        Arg::with_name("wait").long("wait")
                            .help("Process stays in memory until you press the enter key.")
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


        .subcommand(SubCommand::with_name("v0.1/build")
            .about("Runs the given operation file")
            .arg(
                Arg::with_name("in").long("in")
                .min_values(1)
                .multiple(true)
                .help("Replace/add inputs for the operation file")
            )
                        .arg(Arg::with_name("out").long("out").min_values(1).multiple(true)
                            .help("Replace/add outputs for the operation file"))
            .arg(Arg::with_name("demo").long("demo").takes_value(true).possible_values(&["example:200x200_png"]))
            .arg(Arg::with_name("json").long("json").takes_value(true).required_unless("demo"))
                        .arg(Arg::with_name("response").long("response").takes_value(true))
        );
    let matches = app.get_matches();
//        get_matches_from_safe_borrow(std::env::args()).unwrap_or_else(|e| {
//        // Otherwise, write to stderr and exit
//        app.maybe_wait_for_exit(e);
//    });
//

    if let Some(ref matches) = matches.subcommand_matches("v0.1/build") {
        let m : &&clap::ArgMatches = matches;

        let source = if m.is_present("demo"){
            cmd_build::JobSource::NamedDemo(m.value_of("demo").unwrap().to_owned())
        }else {
            cmd_build::JobSource::JsonFile( m.value_of("json").unwrap().to_owned())
        };

        let builder = cmd_build::CmdBuild::parse(source, m.values_of_lossy("in"), m.values_of_lossy("out")).build_maybe();
        builder.write_response_maybe( m.value_of("response")).expect("IO error writing JSON output file. Does the directory exist?");
        builder.write_errors_maybe().expect("Writing to stderr failed!");
        return builder.get_exit_code().unwrap();
    }

    if let Some(ref matches) = matches.subcommand_matches("diagnose") {
        let m : &&clap::ArgMatches = matches;

        if m.is_present("show-compilation-info"){
            println!("{}\n{}\n", s::version::one_line_version(), s::version::all_build_info_pairs());
            return 0;
        }

        if m.is_present("self-test"){
            return self_test::run();
        }
        if m.is_present("wait"){
            let mut input_buf = String::new();
            let input = std::io::stdin().read_line(&mut input_buf).ok().expect("Failed to read from stdin. Are you using --wait in a non-interactive shell?");
            println!("{}", input);
            return 0;
        }
    }
    64
}



