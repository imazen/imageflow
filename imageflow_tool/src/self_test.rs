use std;
use std::convert::AsRef;
use std::fs::{File, create_dir_all};
use std::io::{Write, Read, BufWriter};
use std::path::{Path};
use std::process::{Command, Output};
use std::time::{Duration, Instant};
use std::env;
use fc::clients::stateless;
use s;
use serde_json;
use serde;
use chrono::UTC;

//create dir
//export sample json files
//export sample images
//test a couple remote URLs that we trust to work for years
//use a few embedded ones
// run imageflow in a few different ways

fn write_json<T>(dir: &str, filename: &str, info: T)
    where T: serde::Serialize{
    let path = Path::new(dir).join(filename);
    let mut file = BufWriter::new(File::create(&path).unwrap());
    write!(file, "{}", serde_json::to_string_pretty(&info).unwrap()).unwrap();
}

fn create_blank(dir: &str, filename_without_ext: &str, w: usize, h: usize, encoder: s::EncoderPreset){

    //Invalid read here; the result of create_canvas is not being accessed correctly.
    let req = stateless::BuildRequest{
        inputs: vec![],
        framewise: s::Framewise::Steps(
            vec![
                s::Node::CreateCanvas{ w: w, h: h, format: s::PixelFormat::Bgr24, color: s::Color::Black},
                s::Node::Encode{ io_id: 0, preset: encoder }
            ]
        ),
        export_graphs_to: None //Some(std::path::PathBuf::from(format!("./{}/{}_debug", dir, filename_without_ext)))

    };
    let result = stateless::LibClient::new().build(req).unwrap();
    let ref out: stateless::BuildOutput = result.outputs[0];
    let mut path = Path::new(dir).join(filename_without_ext);
    path.set_extension(&out.file_ext);

    let mut file = BufWriter::new(File::create(&path).unwrap());
    file.write(&out.bytes).unwrap();
}

fn setup(dir: &str){
    write_json(dir, "example1.json", s::Build001::example_with_steps());
    create_blank(dir, "200x200", 200, 200, s::EncoderPreset::libjpegturbo());
    create_blank(dir, "200x200", 200, 200, s::EncoderPreset::libpng32());
    let to_path =  Path::new(dir).join("valgrind_suppressions.txt");
    std::fs::copy("../valgrind_suppressions.txt", to_path).unwrap();
}

fn test(exe: &Path, dir: &str, args: &str, expected_exit_code: i32 ){
    let args_vec = args.split_whitespace().collect::<Vec<&str>>();

    println!("Testing {} {}", exe.to_str().unwrap(), args);
    //change working dir to dir
    let mut cmd = Command::new(exe);
    cmd.args(args_vec.as_slice()).current_dir(dir).env("RUST_BACKTRACE", "1");


//    let output: Output = cmd.output().expect("Failed to start?");
//    std::io::stderr().write(&output.stderr).unwrap();
//    std::io::stdout().write(&output.stdout).unwrap();
//    println!("exit code {:?}", output.status.code());
//    assert_eq!(output.status.code().unwrap(),expected_exit_code);

    let status: std::process::ExitStatus = cmd.status().expect("Failed to start?");

    if status.code() == None{
        //Killed by signal.
        // 11 Segmentation fault
        // 4 illegal instruction 6 abort 8 floating point error

        let mut cmd = Command::new("valgrind");
        cmd.arg("-q").arg("--error-exitcode=9").arg(exe);
        cmd.args(args_vec.as_slice()).current_dir(dir).env("RUST_BACKTRACE", "1");

        println!("{:?}", cmd);

        cmd.status().expect("Failed to start valgrind?");

    }

    println!("exit code {}", status);
    assert_eq!(status.code(),Some(expected_exit_code));

}

pub fn run() -> i32{

    let self_path = std::env::current_exe().expect("For --self-test to work, we need to know the binary's location. env::current_exe failed");

    let dir = format!("./self_tests/{:032}", UTC::now().timestamp());
    create_dir_all(&dir).expect("Failed to create test directory");
    setup(&dir);

    test(self_path.as_ref(), &dir, "v0.1/build --json example1.json --in 200x200.png 200x200.jpg --out out0.jpg --response out0.json", 0);

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


    //If someone can hardlink the current location, this could be used for priviledge escalation.
    //This is a test suite. bah

    println!("Stub self-test");
    0
    //println!("{}\n{}\n", s::version::one_line_version(), s::version::all_build_info_pairs());
}
