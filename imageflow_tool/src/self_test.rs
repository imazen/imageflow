extern crate std;
use fc::for_other_imageflow_crates::preludes::default::*;
extern crate imageflow_core as fc;
extern crate chrono;


use chrono::UTC;
use fc::clients::stateless;
use std::process::{Command, Output};
#[allow(unused_imports)]
use std::time::{Duration, Instant};

// create dir
// export sample json files
// export sample images
// test a couple remote URLs that we trust to work for years
// use a few embedded ones
// run imageflow in a few different ways


// #[derive(Clone, Debug, PartialEq)]
// enum ToolProduct{
//    CrashedUTF8{stderr: String, stdout: String},
//    NoErrorUTF8{stdout: String},
//    UserErrorUTF8{stdout: String, stderr: String, exit_code: i32},
//    GracefulBugUTF8{stdout: String, stderr: String, exit_code: i32},
//    Exited(Output),
//    Other(Output)
// }
//
//
// fn into_product(r: Output) -> ToolProduct{
//    let stdout_str = str::from_utf8(&r.stdout).map(|s| s.to_owned());
//    let stderr_str = str::from_utf8(&r.stderr).map(|s| s.to_owned());
//    if stderr_str.is_err() || stdout_str.is_err(){
//        if r.status.code() == None {
//            return ToolProduct::Other(r);
//        }else{
//            return ToolProduct::Exited(r);
//        }
//    }
//
//    let stderr_present = r.stderr.len() > 0;
//
//    match r.status.code(){
//        Some(65) => ToolProduct::UserErrorUTF8{ stdout: stdout_str.unwrap(), stderr: stderr_str.unwrap(),exit_code: r.status.code().unwrap()},
//        Some(70) => ToolProduct::GracefulBugUTF8{ stdout: stdout_str.unwrap(), stderr: stderr_str.unwrap(),exit_code: r.status.code().unwrap()},
//        None => ToolProduct::CrashedUTF8{ stdout: stdout_str.unwrap(), stderr: stderr_str.unwrap()},
//        Some(0) if !stderr_present => ToolProduct::NoErrorUTF8{ stdout: stdout_str.unwrap()},
//        Some(_) => ToolProduct::Exited(r)
//    }
// }
#[derive(Clone, Debug, PartialEq)]
struct ToolProduct {
    r: Output,
}
fn into_product(r: Output) -> ToolProduct {
    ToolProduct { r: r }
}

impl ToolProduct {
    //    fn status_code(&self) -> Option<i32> {
    //        match *self {
    //            ToolProduct::NoErrorUTF8 { .. } => Some(0),
    //            ToolProduct::UserErrorUTF8 { exit_code, .. } =>
    //                Some(exit_code),
    //            ToolProduct::GracefulBugUTF8 { exit_code, .. } =>
    //                Some(exit_code),
    //            ToolProduct::Exited(ref out) =>
    //                {out.status.code()}
    //            ToolProduct::Other(ref out) => {out.status.code()}
    //            ToolProduct::CrashedUTF8{ .. } => None
    //        }
    //    }

    fn status_code(&self) -> Option<i32> {
        self.r.status.code()
    }
    fn stdout_bytes(&self) -> usize {
        self.r.stdout.len()
    }
    fn stderr_bytes(&self) -> usize {
        self.r.stderr.len()
    }

    fn stderr_str(&self) -> &str {
        std::str::from_utf8(&self.r.stderr)
            .expect("Implement lossy UTF-8 decoding for test results")
    }
    fn stdout_str(&self) -> &str {
        std::str::from_utf8(&self.r.stdout)
            .expect("Implement lossy UTF-8 decoding for test results")
    }
    fn expect_exit_0_no_output(&self, m: &str) -> &ToolProduct {
        if self.stderr_bytes() > 0 || self.stdout_bytes() > 0 || self.status_code() != Some(0) {
            panic!("{}\nExpected exit code 0 and no output to stderr or stdout. Received {:?}",
                   m,
                   self.r);
        }
        &self
    }
    fn expect_status_code(&self, code: Option<i32>) -> &ToolProduct {
        assert_eq!(code, self.status_code());
        &self
    }
    fn expect_stderr_contains(&self, substring: &str) -> &ToolProduct {
        if !self.stderr_str().contains(substring) {
            panic!("Failed to locate substring {:?} within stderr output {}",
                   substring,
                   self.stderr_str());
        }
        self
    }

    fn expect_stdout_contains(&self, substring: &str) -> &ToolProduct {
        if !self.stdout_str().contains(substring) {
            panic!("Failed to locate substring {:?} within stdout output {}",
                   substring,
                   self.stdout_str());
        }
        self
    }
    fn parse_stdout_as<T>(&self) -> std::result::Result<T, serde_json::error::Error>
        where T: serde::Deserialize
    {
        serde_json::from_slice(&self.r.stdout)
    }



    fn dump(&self) -> &ToolProduct {
        let _ = writeln!(&mut std::io::stderr(),
                         "{:?}\n{}",
                         self.r,
                         self.stderr_str());
        self
    }
}


struct TestContext {
    imageflow_tool: PathBuf,
    test_dir: PathBuf,
}

impl TestContext {
    fn create(parent_folder: &Path, tool_location: Option<PathBuf>) -> TestContext {
        let self_path = match tool_location {
            None => std::env::current_exe().expect("For --self-test to work, we need to know the binary's location. env::current_exe failed"),
            Some(p) => p,
        };
        let dir = parent_folder.join(format!("{:032}", UTC::now().timestamp()));

        if let Err(e) = create_dir_all(&dir) {
            panic!("Failed to create directory {:?} due to {:?}", dir, e);
        }

        let to_path = dir.join("valgrind_suppressions.txt");
        std::fs::copy(Path::new("..").join("valgrind_suppressions.txt"), to_path).unwrap();



        TestContext {
            imageflow_tool: self_path,
            test_dir: dir,
        }
    }

    fn exec(&self, args: &str) -> ToolProduct {
        let args_vec = args.split_whitespace().collect::<Vec<&str>>();
        let dir = self.test_dir.as_path();
        let exe = self.imageflow_tool.as_path();

        writeln!(&mut std::io::stderr(),
                 "Testing {} {}",
                 exe.to_str().unwrap(),
                 args);
        // change working dir to dir
        let mut cmd = Command::new(exe);
        cmd.args(args_vec.as_slice()).current_dir(dir).env("RUST_BACKTRACE", "1");
        let output: Output = cmd.output().expect("Failed to start?");

        // Try to debug segfaults
        if output.status.code() == None {

            std::io::stderr().write(&output.stderr).unwrap();
            std::io::stdout().write(&output.stdout).unwrap();
            println!("exit code {:?}", output.status.code());

            // Killed by signal.
            // 11 Segmentation fault
            // 4 illegal instruction 6 abort 8 floating point error

            let mut cmd = Command::new("valgrind");
            cmd.arg("-q").arg("--error-exitcode=9").arg(exe);
            cmd.args(args_vec.as_slice()).current_dir(dir).env("RUST_BACKTRACE", "1");

            println!("{:?}", cmd);

            let _ = cmd.status(); //.expect("Failed to start valgrind?");
        }

        into_product(output)
    }

    fn write_json<T>(&self, filename: &str, info: T)
        where T: serde::Serialize
    {
        let path = self.test_dir.join(filename);
        let mut file = BufWriter::new(File::create(&path).unwrap());
        write!(file, "{}", serde_json::to_string_pretty(&info).unwrap()).unwrap();
    }

    fn create_blank(&self,
                    filename_without_ext: &str,
                    w: usize,
                    h: usize,
                    encoder: s::EncoderPreset) {

        // Invalid read here; the result of create_canvas is not being accessed correctly.
        let req = stateless::BuildRequest {
            inputs: vec![],
            framewise: s::Framewise::Steps(vec![s::Node::CreateCanvas {
                                                    w: w,
                                                    h: h,
                                                    format: s::PixelFormat::Bgr24,
                                                    color: s::Color::Black,
                                                },
                                                s::Node::Encode {
                                                    io_id: 0,
                                                    preset: encoder,
                                                }]),
            export_graphs_to: None, /* Some(std::path::PathBuf::from(format!("./{}/{}_debug", dir, filename_without_ext))) */
        };
        let result = stateless::LibClient::new().build(req).unwrap();
        let ref out: stateless::BuildOutput = result.outputs[0];
        let mut path = self.test_dir.join(filename_without_ext);
        path.set_extension(&out.file_ext);

        let mut file = BufWriter::new(File::create(&path).unwrap());
        file.write(&out.bytes).unwrap();
    }
}



pub fn run(tool_location: Option<PathBuf>) -> i32 {

    let c = TestContext::create(Path::new("self_tests"), tool_location);
    // encapsulate scenario/example for reuse


    {
        c.write_json("example1.json", s::Build001::example_with_steps());
        c.create_blank("200x200", 200, 200, s::EncoderPreset::libjpegturbo());
        c.create_blank("200x200", 200, 200, s::EncoderPreset::libpng32());

        c.exec("v0.1/build --json example1.json --in 200x200.png 200x200.jpg --out out0.jpg --response out0.json").expect_exit_0_no_output("");
        // TODO: Verify out0.json exists and was created
    }
    {
        c.write_json("example2.json", s::Build001::example_with_steps());
        c.create_blank("200x200", 200, 200, s::EncoderPreset::libjpegturbo());
        c.create_blank("200x200", 200, 200, s::EncoderPreset::libpng32());

        let result =
            c.exec("v0.1/build --json example2.json --in 200x200.png 200x200.jpg --out out0.jpg");

        result.expect_status_code(Some(0));

        let resp: s::Response001 = result.parse_stdout_as::<s::Response001>().unwrap();
        match resp.data {
            s::ResponsePayload::BuildResult(info) => {
                println!("encodes: {:?}", &info.encodes);
                assert!(info.encodes.len() >= 1);
                let encode: &s::EncodeResult = &info.encodes[0];
                assert_eq!(encode.w, 100);
            }
            _ => panic!("Build result not sent"),
        }

    }

    // It seems that Clap always uses status code 1 to indicate a parsing failure
    c.exec("bad command").expect_status_code(Some(1));

    // Write something unexpected, but valid JSON
    c.write_json("random_object.json", s::PngBitDepth::Png24);

    c.exec("v0.1/build --json random_object.json")
        .expect_status_code(Some(65))
        .expect_stderr_contains("InvalidType(Str)");
    // .expect_stdout_contains("")   ; //todo: should respond with JSON version of error message

    {
        // Test having both input and canvas point to the same bitmap
        let a = fluent::fluently().canvas_bgra32(10, 10, s::Color::Black);
        let b = a.branch().copy_rect_from(a.branch(), 0, 0, 5, 5, 0, 0);
        c.write_json("bad__canvas_and_input_equal.json",
                     s::Build001 {
                         builder_config: None,
                         framewise: b.builder().to_framewise(),
                         io: vec![],
                     });
        c.exec("v0.1/build --json bad__canvas_and_input_equal.json").dump();
    }

    {
        // Test a cycle
        let mut nodes = HashMap::new();
        nodes.insert("0".to_owned(), s::Node::FlipH);
        nodes.insert("1".to_owned(), s::Node::FlipV);
        let g = s::Graph {
            edges: vec![s::Edge {
                            from: 0,
                            to: 1,
                            kind: s::EdgeKind::Input,
                        },
                        s::Edge {
                            from: 1,
                            to: 0,
                            kind: s::EdgeKind::Input,
                        }],
            nodes: nodes,
        };
        c.write_json("bad__cycle.json",
                     s::Build001 {
                         builder_config: None,
                         framewise: s::Framewise::Graph(g),
                         io: vec![],
                     });
        c.exec("v0.1/build --json bad__cycle.json").dump();
    }
    {
        // Test a loop TODO: Fix
        let mut nodes = HashMap::new();
        nodes.insert("0".to_owned(), s::Node::FlipH);
        let g = s::Graph {
            edges: vec![s::Edge {
                            from: 0,
                            to: 0,
                            kind: s::EdgeKind::Input,
                        }],
            nodes: nodes,
        };
        c.write_json("bad__loop.json",
                     s::Build001 {
                         builder_config: None,
                         framewise: s::Framewise::Graph(g),
                         io: vec![],
                     });
        let result = c.exec("v0.1/build --json bad__loop.json");
        // Stack overflow
        assert_eq!(result.status_code(), None);
    }

    // Test a cycle
    //    let a = fluent::fluently().create_canvas(10,10,s::PixelFormat::Bgr24, s::Color::Black);
    //    let b = fluent::fluently().create_canvas(10,10,s::PixelFormat::Bgr24, s::Color::Black);
    //
    //    let c = a.branch().copy_rect_from(b);
    //
    //    .copy_rect_from()



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



    println!("Stub self-test");
    0
}
