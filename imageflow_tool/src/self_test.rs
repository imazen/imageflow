extern crate std;
use fc::for_other_imageflow_crates::preludes::default::*;
use std::slice::SliceConcatExt;
extern crate imageflow_core as fc;
extern crate chrono;
extern crate curl;
extern crate os_type;

use self::curl::easy::Easy;


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
    #[allow(dead_code)]
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
        if code != self.status_code(){
            self.dump();
            assert_eq!(code, self.status_code());
        }
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
    fn create_for_examples(parent_folder_str: &str, tool_location: Option<PathBuf>) -> TestContext {
        let parent_folder = Path::new(parent_folder_str);
        let self_path = match tool_location {
            None => std::env::current_exe().expect("For --self-test to work, we need to know the binary's location. env::current_exe failed"),
            Some(p) => p,
        };
        let dir = parent_folder;
        if let Err(e) = create_dir_all(&dir) {
            panic!("Failed to create directory {:?} due to {:?}", dir, e);
        }

        TestContext {
            imageflow_tool: self_path,
            test_dir: dir.to_owned(),
        }
    }

    fn create(parent_folder: &Path, tool_location: Option<PathBuf>) -> TestContext {
        let self_path = match tool_location {
            None => std::env::current_exe().expect("For --self-test to work, we need to know the binary's location. env::current_exe failed"),
            Some(p) => p,
        };
        let dir = parent_folder.join(format!("{:032}", UTC::now().timestamp()));

        if let Err(e) = create_dir_all(&dir) {
            panic!("Failed to create directory {:?} due to {:?}", dir, e);
        }


        TestContext {
            imageflow_tool: self_path,
            test_dir: dir,
        }
    }

    fn create_valgrind_suppressions(&self) -> bool{
        let mut dir = self.test_dir.clone();
        loop{
            let potential = dir.join("valgrind_suppressions.txt");
            if !potential.exists(){
                dir = match dir.parent(){
                    Some(v) => v.to_owned(),
                    None => { break; }
                };
            }else{
                let to_path = self.test_dir.join("valgrind_suppressions.txt");
                std::fs::copy(potential, to_path).unwrap();
                return true;
            }
        }
        false
    }
    pub fn subfolder(&self, subfolder: &Path) -> TestContext{
        let new_dir = self.test_dir.join(subfolder);
        if let Err(e) = create_dir_all(&new_dir) {
            panic!("Failed to create directory {:?} due to {:?}", &new_dir, e);

        }

        TestContext{
            test_dir: new_dir,
            imageflow_tool: self.imageflow_tool.clone()
        }
    }

    fn exec(&self, args: &str) -> ToolProduct {
        let full = format!("{} {}", &self.imageflow_tool.to_str().unwrap(), args);
        self.exec_full(&full)
    }
    fn exec_full(&self, full_invocation: &str) -> ToolProduct {
        let mut parts_vec = full_invocation.split_whitespace().collect::<Vec<&str>>();
        let _ = parts_vec.remove(0);

        let args_vec = parts_vec;
        let dir = self.test_dir.as_path();
        let exe = self.imageflow_tool.as_path();

        let _ = writeln!(&mut std::io::stderr(),
                 "Executing from folder {}\n{}",
                 dir.to_str().unwrap(),
                 full_invocation);
        // change working dir to dir
        let mut cmd = Command::new(exe);
        cmd.args(args_vec.as_slice()).current_dir(dir).env("RUST_BACKTRACE", "1");
        let output: Output = cmd.output().expect("Failed to start?");
        let _ = writeln!(&mut std::io::stderr(),
                 "exit code {:?}", output.status.code());

        // Try to debug segfaults
        if output.status.code() == None {
            self.create_valgrind_suppressions();

            std::io::stderr().write(&output.stderr).unwrap();
            std::io::stdout().write(&output.stdout).unwrap();
            println!("exit code {:?}", output.status.code());

            // Killed by signal.
            // 11 Segmentation fault
            // 4 illegal instruction 6 abort 8 floating point error
            let _ = writeln!(&mut std::io::stderr(),
                             "Starting valgrind from within self-test:");
            let mut cmd = Command::new("valgrind");
            cmd.arg("-q").arg("--error-exitcode=9").arg(exe);
            cmd.args(args_vec.as_slice()).current_dir(dir).env("RUST_BACKTRACE", "1");

            println!("{:?}", cmd);

            let _ = cmd.status(); //.expect("Failed to start valgrind?");
        }

        into_product(output)
    }

    fn write_json<T>(&self, filename: &str, info: &T)
        where T: serde::Serialize
    {
        let path = self.test_dir.join(filename);
        let mut file = BufWriter::new(File::create(&path).unwrap());
        write!(file, "{}", serde_json::to_string_pretty(info).unwrap()).unwrap();
    }

    fn create_blank(&self,
                    filename_without_ext: &str,
                    w: u32,
                    h: u32,
                    encoder: s::EncoderPreset) {
        let out = BlankImage{
            w: w,
            h: h,
            encoding: encoder,
            color: s::Color::Black
        }.generate();


        let mut path = self.test_dir.join(filename_without_ext);
        path.set_extension(&out.file_ext);

        self.write_file(path.file_name().unwrap().to_str().unwrap(), &out.bytes);
    }

    fn write_file(&self, filename: &str, bytes: &[u8]){
        let path = self.test_dir.join(filename);
        let mut file = BufWriter::new(File::create(&path).unwrap());
        file.write(bytes).unwrap();
    }

    fn read_file_str(&self, filename: &str) -> String{
        let path = self.test_dir.join(filename);
        let mut file = File::open(&path).unwrap();
        let mut contents = String::new();
        file.read_to_string( &mut contents).unwrap();
        contents
    }
}

fn fetch_url(url: &str) -> Vec<u8>{

    let mut dst = Vec::new();
    {
        let mut easy = Easy::new();
        easy.url(&url).unwrap();

        let mut transfer = easy.transfer();
        transfer.write_function(|data| {
            dst.extend_from_slice(data);
            Ok(data.len())
        })
            .unwrap();
        transfer.perform().unwrap();
    }
    dst

}
#[derive(Clone,Debug,PartialEq)]
enum ImageSource{
    Url(String),
    Blank(BlankImage)
}


#[derive(Clone,Debug,PartialEq)]
struct BlankImage{
 pub w: u32,
 pub h: u32,
 pub color: s::Color,
 pub encoding: s::EncoderPreset
}

impl BlankImage{
    fn generate(&self) -> stateless::BuildOutput{
        // Invalid read here; the result of create_canvas is not being accessed correctly.
        let req = stateless::BuildRequest {
            inputs: vec![],
            framewise: s::Framewise::Steps(vec![s::Node::CreateCanvas {
                w: self.w as usize,
                h: self.h as usize,
                format: s::PixelFormat::Bgr24,
                color: self.color.clone(),
            },
            s::Node::Encode {
                io_id: 0,
                preset: self.encoding.clone(),
            }]),
            export_graphs_to: None, /* Some(std::path::PathBuf::from(format!("./{}/{}_debug", dir, filename_without_ext))) */
        };
        let result = stateless::LibClient::new().build(req).unwrap();
        result.outputs.into_iter().next().unwrap()
    }
}

#[derive(Clone,Debug,PartialEq)]
enum ReplacementInput{
    File{path: String, source: ImageSource},
    Url(String),
}
impl ReplacementInput{
    pub fn prepare(&self, c: &TestContext){
        match self{
            &ReplacementInput::File{ref path, ref source} => {
                let bytes = match source{
                    &ImageSource::Url(ref url) => {
                        fetch_url(&url)
                    },
                    &ImageSource::Blank(ref blank) => {
                        blank.generate().bytes
                    }
                };
                c.write_file(path, &bytes);
            }
            _ => {}
        }
    }
    pub fn parameter(& self) -> String{
        match self{
            &ReplacementInput::File{ref path, ..} => path.to_owned(),
            &ReplacementInput::Url(ref str) => str.to_owned()
        }
    }
}

#[derive(Clone,Debug,PartialEq)]
struct ReplacementOutput{
    pub io_id: i32,
    pub value: OutputDestination
}
impl ReplacementOutput{
    pub fn file(io_id: i32, path: &'static str) -> ReplacementOutput{
        ReplacementOutput{
            io_id: io_id,
            value: OutputDestination::File{path: path.to_owned()}
        }
    }
    pub fn b64(io_id: i32) -> ReplacementOutput{
        ReplacementOutput{
            io_id: io_id,
            value: OutputDestination::Base64
        }
    }

    pub fn parameter(&self) -> String{
        match self.value{
            OutputDestination::File{ref path} => {format!("{} {}", self.io_id, path)},
            OutputDestination::Base64 => format!("{} base64:", self.io_id) //We would quote this in real life, but we're splitting on whitespace
        }
    }
}

#[derive(Clone,Debug,PartialEq)]
enum OutputDestination{
    File{path: String},
    Base64,
}

//enum Expectation<'a>{
//    ImageExists(&'a ReplacementOutput),
//    ImageMatches{dest: &'a ReplacementOutput, w: Option<u32>, h: Option<u32>, content_type: Option<&'static str>},
//
//}
struct BuildScenario{
    pub description: &'static str,
    pub slug: &'static str,
    pub recipe: s::Build001,
    pub new_inputs: Vec<ReplacementInput>,
    pub new_outputs: Vec<ReplacementOutput>,
    pub json_out: Option<&'static str>,
    pub expectations: Option<ScenarioExpectations>
}
//expect that output file exists
//expect that output bytes parse
//expect that output json exists, parses, and represents success

struct ScenarioExpectations{
    status_code: Option<i32>
}
fn scenario_export_4() -> BuildScenario{
    let preset = s::EncoderPreset::libjpegturbo_q(Some(90));
    let s = fluent::fluently().decode(0);
    let v1 = s.branch().constrain_within(Some(1600), None, None);
    let v2 = v1.branch().constrain_within(Some(1200), None, None);
    let v3 = v1.branch().constrain_within(Some(800), None, None);
    let v4 = v2.branch().constrain_within(Some(400), None, None);
    let framewise = v1.encode(1,preset.clone()).builder().with(v2.encode(2,preset.clone())).with(v3.encode(3,preset.clone())).with(v4.encode(4, preset.clone())).to_framewise();

    BuildScenario{
        description: "Generate 4 sizes of a jpeg",
        slug: "export_4_sizes",
        recipe: framewise.wrap_in_build_0_1(),
        new_inputs: vec![ReplacementInput::Url("http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())],
        new_outputs: vec![ReplacementOutput::file(1, "waterhouse_w1600.jpg"),
        ReplacementOutput::file(2, "waterhouse_w1200.jpg"),
        ReplacementOutput::file(3, "waterhouse_w800.jpg"),
        ReplacementOutput::file(4, "waterhouse_w400.jpg"),],
        json_out: Some("operation_result.json"),
        expectations: Some(ScenarioExpectations{status_code: Some(0)})
    }
}

const BLUE_PNG32_200X200_B64:&'static str = "iVBORw0KGgoAAAANSUhEUgAAAMgAAADICAYAAACtWK6eAAABiUlEQVR42u3TgRAAQAgAsA/qkaKLK48EIug2h8XP6gesQhAQBAQBQUAQEAQEAUFAEBAEEAQEAUFAEBAEBAFBQBAQBAQRBAQBQUAQEAQEAUFAEBAEBAEEAUFAEBAEBAFBQBAQBAQBQQBBQBAQBAQBQUAQEAQEAUEAQUAQEAQEAUFAEBAEBAFBQBBAEBAEBAFBQBAQBAQBQUAQQBAQBAQBQUAQEAQEAUFAEBAEEAQEAUFAEBAEBAFBQBAQBAQRBAQBQUAQEAQEAUFAEBAEBAEEAUFAEBAEBAFBQBAQBAQBQQQBQUAQEAQEAUFAEBAEBAFBAEFAEBAEBAFBQBAQBAQBQUAQQBAQBAQBQUAQEAQEAUFAEEAQEAQEAUFAEBAEBAFBQBAQBBAEBAFBQBAQBAQBQUAQEAQQBAQBQUAQEAQEAUFAEBAEBAEEAUFAEBAEBAFBQBAQBAQBQQQBQUAQEAQEAUFAEBAEBAFBAEFAEBAEBAFBQBAQBAQBQUAQQUAQEAQEAUFAEBAEBIGLBkZ+sahOjkyUAAAAAElFTkSuQmCC";

fn scenario_pure_json() -> BuildScenario{
    let framewise = fluent::fluently()
        .decode(0)
        .constrain_within(Some(40), Some(40), Some(s::ConstraintResamplingHints::with(None, Some(25f32))))
        .encode(1, s::EncoderPreset::libpng32()).builder().to_framewise();

    BuildScenario{
        description: "Base64 encoding permits you to embed images in the json recipe itself",
        slug: "pure_json",
        recipe: framewise.wrap_in_build_0_1()
            .replace_io(0, s::IoEnum::Base64(BLUE_PNG32_200X200_B64.to_owned()))
            .replace_io(1, s::IoEnum::OutputBase64),
        new_inputs: vec![],
        new_outputs: vec![],
        json_out: Some("operation_result.json"),
        expectations: Some(ScenarioExpectations{status_code: Some(0)})
    }
}
fn scenario_response_stdout() -> BuildScenario{
    let framewise = fluent::fluently()
        .decode(0)
        .constrain_within(Some(40), Some(40), Some(s::ConstraintResamplingHints::with(None, Some(25f32))))
        .encode(1, s::EncoderPreset::libpng32()).builder().to_framewise();

    BuildScenario{
        description: "The JSON result is sent to stdout if --response [filename] is not specified.",
        slug: "pure_json_to_stdout",
        recipe: framewise.wrap_in_build_0_1()
            .replace_io(0, s::IoEnum::Base64(BLUE_PNG32_200X200_B64.to_owned()))
            .replace_io(1, s::IoEnum::OutputBase64),
        new_inputs: vec![],
        new_outputs: vec![],
        json_out: None,
        expectations: Some(ScenarioExpectations{status_code: Some(0)})
    }
}

fn scenario_laundry_list() -> BuildScenario{
    let chain = fluent::fluently()
        .to(s::Node::Decode{io_id:0, commands: Some(vec![s::DecoderCommand::JpegDownscaleHints(s::JpegIDCTDownscaleHints{
            gamma_correct_for_srgb_during_spatial_luma_scaling: Some(false),
            scale_luma_spatially: Some(false),
            width: 1600,
            height:1600
    })])})
        .constrain_within(Some(1400), None,None)
        .constrain_within(Some(1400), Some(1400), Some(s::ConstraintResamplingHints::with(Some(s::Filter::CatmullRom), Some(40f32))))
        .to(s::Node::Resample2D {
            w: 800,
            h: 800,
            down_filter: Some(s::Filter::Robidoux),
            up_filter: Some(s::Filter::Ginseng),
            hints: Some(s::ResampleHints {
                sharpen_percent: Some(10f32),
                prefer_1d_twice: None,
            }),
        })
        .to(s::Node::ApplyOrientation{flag: 7}).flip_horizontal().flip_vertical().transpose().rotate_90().rotate_180().rotate_270()
        .to(s::Node::FillRect {
            x1: 0,
            y1: 0,
            x2: 8,
            y2: 8,
            color: s::Color::Transparent,
        }).to(                              s::Node::ExpandCanvas {
        left: 10,
        top: 10,
        right: 10,
        bottom: 10,
        color: s::Color::Srgb(s::ColorSrgb::Hex("FFEECCFF".to_owned())),
    }).to(s::Node::Crop {
        x1: 10,
        y1: 10,
        x2: 650,
        y2: 490,
    }).encode(1, s::EncoderPreset::Libpng{
        depth: Some(s::PngBitDepth::Png24),
        matte: Some(s::Color::Srgb(s::ColorSrgb::Hex("9922FF".to_owned()))),
        zlib_compression: Some(7)
    });

    let framewise = chain.builder().to_framewise();

    BuildScenario{
        description: "A rather nonsensical enumeration of operations",
        slug: "laundry_list",
        recipe: framewise.wrap_in_build_0_1(),
        new_inputs: vec![ReplacementInput::File{path: "blank3200.jpg".to_owned(), source: ImageSource::Blank(BlankImage{w: 3200, h:3200, color: s::Color::Black, encoding: s::EncoderPreset::libjpegturbo_q(Some(5))})}
],
        new_outputs: vec![ReplacementOutput::file(1,"wat.jpg")],
        json_out: None,
        expectations: Some(ScenarioExpectations{status_code: Some(0)})
    }
}


fn scenario_request_base64() -> BuildScenario{
    let framewise = fluent::fluently()
        .decode(0)
        .constrain_within(Some(5), Some(5), Some(s::ConstraintResamplingHints::with(None, Some(25f32))))
        .encode(1, s::EncoderPreset::libpng32()).builder().to_framewise();

    BuildScenario{
        description: "base64: is a keyword: --out \"base64:\" causes the result to be base64 encoded into the response JSON",
        slug: "give_me_base64",
        recipe: framewise.wrap_in_build_0_1(),
        new_inputs: vec![ReplacementInput::File{path: "rings2.png".to_owned(), source: ImageSource::Url("http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/rings2.png".to_owned())}],
        new_outputs: vec![ReplacementOutput::b64(1)],
        json_out: Some("operation_result.json"),
        expectations: Some(ScenarioExpectations{status_code: Some(0)})
    }
}




fn scenarios() -> Vec<BuildScenario>{
    vec![
    scenario_laundry_list(),
    scenario_export_4(),
    scenario_pure_json(),
    scenario_response_stdout(),
    scenario_request_base64()
    ]
}


impl TestContext {

    fn default_script_name() -> &'static str{
        match os_type::current_platform() {
            os_type::OSType::Windows => "run_recipe.bat",
            _ => "run_recipe.sh"
        }
    }
    fn prepare_scenario(&self, item: &BuildScenario) -> TestContext {
        let c = self.subfolder(Path::new(item.slug));
        println!("Preparing example {} in \n{:?}\n\n{}", item.slug,c.test_dir, item.description);
        for input in item.new_inputs.as_slice().iter(){
            input.prepare(&c);
        }
        let json_fname = format!("{}.json", item.slug);
        c.write_json(&json_fname, &item.recipe);

        let mut command = format!("{} v0.1/build --json {}", self.imageflow_tool.as_path().to_str().unwrap(), json_fname);
        if item.new_inputs.len() > 0 {
            let arg = format!(" --in {}", item.new_inputs.as_slice().iter().map(|i| i.parameter()).collect::<Vec<String>>().join(" "));
            command.push_str(&arg);
        }
        if item.new_outputs.len() > 0 {
            let arg = format!(" --out {}", item.new_outputs.as_slice().iter().map(|i| i.parameter()).collect::<Vec<String>>().join(" "));
            command.push_str(&arg);
        }
        if let Some(ref outfile) = item.json_out{
            let arg = format!(" --response {}", outfile);
            command.push_str(&arg);
        }

        c.write_file(TestContext::default_script_name(), &command.as_bytes());
        c
    }

    fn run_scenario(&self, item: &BuildScenario) -> ToolProduct {
        let c = self.subfolder(Path::new(item.slug));
        println!("Running example {} in \n{:?}\n\n{}", item.slug, c.test_dir, item.description);

        let full_command = c.read_file_str(TestContext::default_script_name());

        let product = c.exec_full(&full_command);

        if let Some(ScenarioExpectations{ref status_code}) = item.expectations{
            product.expect_status_code(status_code.clone());
        }
        product
    }

}

pub fn export_examples(tool_location: Option<PathBuf>){
    let c = TestContext::create_for_examples("examples", tool_location);
    for example in scenarios(){
        c.prepare_scenario(&example);
    }
}
pub fn run_examples(tool_location: Option<PathBuf>){
    let c = TestContext::create_for_examples("examples", tool_location);
    for example in scenarios(){
        c.run_scenario(&example);
    }
}

pub fn run(tool_location: Option<PathBuf>) -> i32 {

    let c = TestContext::create(Path::new("self_tests"), tool_location);
    // encapsulate scenario/example for reuse
    for example in scenarios(){
        c.prepare_scenario(&example);
        c.run_scenario(&example);
    }
    {
        c.exec("diagnose --show-compilation-info").expect_status_code(Some(0));
        c.exec("--version").expect_status_code(Some(0));
        c.exec("-V").expect_status_code(Some(0));
    }
    {
        let recipe = s::Build001::example_with_steps();
        c.write_json("example1.json", &recipe);
        c.create_blank("200x200", 200, 200, s::EncoderPreset::libjpegturbo());
        c.create_blank("200x200", 200, 200, s::EncoderPreset::libpng32());

        c.exec("v0.1/build --json example1.json --in 200x200.png 200x200.jpg --out out0.jpg --response out0.json").expect_exit_0_no_output("");
        // TODO: Verify out0.json exists and was created
    }
    {
        let recipe =  s::Build001::example_with_steps();
        c.write_json("example2.json",&recipe);
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
    {
        let recipe = fluent::fluently().decode(0).constrain_within(Some(60), Some(45), None).encode(1, s::EncoderPreset::libjpegturbo()).to_build_0_1();
        c.write_json("example2.json", &recipe);
        c.create_blank("200x200", 200, 200, s::EncoderPreset::libjpegturbo());

        let result =
        c.exec("v0.1/build --json example2.json --in 200x200.jpg --out out3.jpg");

        result.expect_status_code(Some(0));

        let resp: s::Response001 = result.parse_stdout_as::<s::Response001>().unwrap();
        match resp.data {
            s::ResponsePayload::BuildResult(info) => {

                assert!(info.encodes.len() == 1);
                let encode: &s::EncodeResult = &info.encodes[0];
                assert_eq!(encode.w, 45);
                assert_eq!(encode.h, 45);
            }
            _ => panic!("Build result not sent"),
        }

    }


    // It seems that Clap always uses status code 1 to indicate a parsing failure
    c.exec("bad command").expect_status_code(Some(1));

    // Write something unexpected, but valid JSON
    c.write_json("random_object.json", &s::PngBitDepth::Png24);

    c.exec("v0.1/build --json random_object.json")
        .expect_status_code(Some(65))
        .expect_stderr_contains("InvalidType(Str)");
    // .expect_stdout_contains("")   ; //todo: should respond with JSON version of error message

    {
        // Test having both input and canvas point to the same bitmap
        // This should fail
        let a = fluent::fluently().canvas_bgra32(10, 10, s::Color::Black);
        let b = a.branch().copy_rect_from(a.branch(), 0, 0, 5, 5, 0, 0);
        let recipe = s::Build001 {
            builder_config: None,
            framewise: b.builder().to_framewise(),
            io: vec![]};
            c.write_json("bad__canvas_and_input_equal.json",&recipe);
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
        let recipe = s::Build001 {
            builder_config: None,
            framewise: s::Framewise::Graph(g),
            io: vec![],
        };
        c.write_json("bad__cycle.json",&recipe);
        c.exec("v0.1/build --json bad__cycle.json").dump();
    }
    {
        // Test invalid edges (FlipV doesn't take a canvas)
//        let mut nodes = HashMap::new();
//        nodes.insert("0".to_owned(), s::Node::FlipH);
//        nodes.insert("1".to_owned(), s::Node::FlipV);
//        nodes.insert("2".to_owned(), s::Node::FlipV);
//        let g = s::Graph {
//            edges: vec![s::Edge {
//                from: 0,
//                to: 1,
//                kind: s::EdgeKind::Input,
//            },
//            s::Edge {
//                from: 2,
//                to: 1,
//                kind: s::EdgeKind::Canvas,
//            }],
//            nodes: nodes,
//        };
//        let recipe = s::Build001 {
//            builder_config: None,
//            framewise: s::Framewise::Graph(g),
//            io: vec![],
//        };
//        c.write_json("bad__node_inputs.json",&recipe);
//        c.exec("v0.1/build --json bad__node_inputs.json").dump();
    }
    {
        // Test a loop TODO: Fix
//        let mut nodes = HashMap::new();
//        nodes.insert("0".to_owned(), s::Node::FlipH);
//        let g = s::Graph {
//            edges: vec![s::Edge {
//                            from: 0,
//                            to: 0,
//                            kind: s::EdgeKind::Input,
//                        }],
//            nodes: nodes,
//        };
//        let recipe = s::Build001 {
//            builder_config: None,
//            framewise: s::Framewise::Graph(g),
//            io: vec![],
//        };
//        c.write_json("bad__loop.json",&recipe);
//        let _ = c.exec("v0.1/build --json bad__loop.json");
        // Stack overflow. None on linux, Some(1073741571) on windows
        //assert_eq!(result., None);
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



    println!("Self-test complete. All tests passed.");
    0
}
