extern crate std;
extern crate imageflow_core;
use self::imageflow_core::for_other_imageflow_crates::preludes::default::*;

//extern crate imageflow_helpers;

use imageflow_helpers::process_testing::*;
use self::imageflow_core::test_helpers::*;
use self::imageflow_core::test_helpers::process_testing::ProcTestContextExtras;
use self::imageflow_core::test_helpers::process_testing::ProcOutputExtras;



#[derive(Clone,Debug,PartialEq)]
enum ReplacementInput{
    File{path: String, source: TestImageSource
    },
    Url(String),
}
impl ReplacementInput{
    pub fn prepare(&self, c: &ProcTestContext){
        #[cfg_attr(feature = "cargo-clippy", allow(single_match))]
        match *self{
            ReplacementInput::File{ref path, ref source} => {
                let bytes = source.get_bytes();
                c.write_file(path, &bytes);
            }
            _ => {}
        }
    }
    pub fn parameter(& self) -> String{
        match *self{
            ReplacementInput::File{ref path, ..} => path.to_owned(),
            ReplacementInput::Url(ref str) => str.to_owned()
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
            io_id,
            value: OutputDestination::File{path: path.to_owned()}
        }
    }
    pub fn b64(io_id: i32) -> ReplacementOutput{
        ReplacementOutput{
            io_id,
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
trait TestScenario{
    fn description() -> &'static str;
    fn slug() -> &'static str;

}

trait TestExpectations{

}

struct BuildScenario{
    pub description: &'static str,
    pub slug: &'static str,
    pub recipe: s::Build001,
    pub new_inputs: Vec<ReplacementInput>,
    pub new_outputs: Vec<ReplacementOutput>,
    pub json_out: Option<&'static str>,
    pub expectations: Option<ScenarioExpectations>
}

impl BuildScenario{

    fn default_script_name() -> &'static str{
        if cfg!(target_os="windows") {
            "run_recipe.bat"
        }else{
            "run_recipe.sh"
        }
    }
    fn prepare_scenario(&self, context: &ProcTestContext) -> ProcTestContext {
        let c = context.subfolder_context(Path::new(self.slug));
        println!("Preparing example {} in \n{:?}\n\n{}", self.slug, c.working_dir(), self.description);
        for input in self.new_inputs.as_slice().iter(){
            input.prepare(&c);
        }
        let json_fname = format!("{}.json", self.slug);
        c.write_json(&json_fname, &self.recipe);

        let mut command = format!("{} v0.1/build --json {}", c.bin_location().to_str().unwrap(), json_fname);
        if !self.new_inputs.is_empty() {
            let arg = format!(" --in {}", self.new_inputs.as_slice().iter().map(|i| i.parameter()).collect::<Vec<String>>().join(" "));
            command.push_str(&arg);
        }
        if !self.new_outputs.is_empty() {
            let arg = format!(" --out {}", self.new_outputs.as_slice().iter().map(|i| i.parameter()).collect::<Vec<String>>().join(" "));
            command.push_str(&arg);
        }
        if let Some(outfile) = self.json_out{
            let arg = format!(" --response {}", outfile);
            command.push_str(&arg);
        }

        c.write_file(Self::default_script_name(), command.as_bytes());
        c
    }

    fn run_scenario(&self, context: &ProcTestContext) -> ProcOutput {
        let c = context.subfolder_context(Path::new(self.slug));
        println!("Running example {} in \n{:?}\n\n{}", self.slug, c.working_dir(), self.description);

        let full_command = c.read_file_str(Self::default_script_name());

        let product = c.exec_full(&full_command);

        if let Some(ScenarioExpectations{status_code}) = self.expectations{
            product.expect_status_code(status_code);
        }
        product
    }

}
//expect that output file exists
//expect that output bytes parse
//expect that output json exists, parses, and represents success

struct ScenarioExpectations{
    status_code: Option<i32>
}
fn scenario_export_4() -> BuildScenario{
    let preset = s::EncoderPreset::libjpeg_turbo_q(Some(90));
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
        .constrain_within(Some(40), Some(40), Some(s::ResampleHints::with(None, Some(25f32))))
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
        .constrain_within(Some(40), Some(40), Some(s::ResampleHints::with(None, Some(25f32))))
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
        .constrain_within(Some(1400), Some(1400), Some(s::ResampleHints::with(Some(s::Filter::CatmullRom), Some(40f32))))
        .to(s::Node::Resample2D {
            w: 800,
            h: 800,
            hints: Some(s::ResampleHints {
                sharpen_percent: Some(10f32),
                background_color: None,
                resample_when: None,
                down_filter: Some(s::Filter::Robidoux),
                up_filter: Some(s::Filter::Ginseng),
                scaling_colorspace: Some(s::ScalingFloatspace::Linear),
                sharpen_when: None
            }),
        })
        .to(s::Node::RegionPercent {x1: -1f32, y1: -1f32, x2: 101f32, y2: 101f32, background_color: s::Color::Transparent})
        .to(s::Node::Region {x1: -1, y1: -1, x2: 800, y2: 800, background_color: s::Color::Transparent})
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
        new_inputs: vec![ReplacementInput::File{path: "blank3200.jpg".to_owned(), source: TestImageSource::Blank(BlankImage{w: 3200, h:3200, color: s::Color::Black, encoding: s::EncoderPreset::libjpeg_turbo_q(Some(5))})}
],
        new_outputs: vec![ReplacementOutput::file(1,"wat.jpg")],
        json_out: None,
        expectations: Some(ScenarioExpectations{status_code: Some(0)})
    }
}


fn scenario_request_base64() -> BuildScenario{
    let framewise = fluent::fluently()
        .decode(0)
        .constrain_within(Some(5), Some(5), Some(s::ResampleHints::with(None, Some(25f32))))
        .encode(1, s::EncoderPreset::libpng32()).builder().to_framewise();

    BuildScenario{
        description: "base64: is a keyword: --out \"base64:\" causes the result to be base64 encoded into the response JSON",
        slug: "give_me_base64",
        recipe: framewise.wrap_in_build_0_1(),
        new_inputs: vec![ReplacementInput::File{path: "rings2.png".to_owned(), source: TestImageSource::Url("http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/rings2.png".to_owned())}],
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


pub fn export_examples(tool_location: Option<PathBuf>){
    let c = ProcTestContext::create("examples", tool_location);
    for example in scenarios(){
        example.prepare_scenario(&c);
    }
}
pub fn run_examples(tool_location: Option<PathBuf>){
    let c = ProcTestContext::create("examples", tool_location);
    for example in scenarios(){
        example.run_scenario(&c);
    }
}


pub fn run(tool_location: Option<PathBuf>) -> i32 {

    let c = ProcTestContext::create_timestamp_subdir_within(std::env::current_dir().unwrap().join("self_tests"), tool_location);
    // encapsulate scenario/example for reuse
    for example in scenarios() {
        example.prepare_scenario(&c);
        example.run_scenario(&c);
    }
    {
        c.exec("diagnose --show-compilation-info").expect_status_code(Some(0));
        c.exec("--version").expect_status_code(Some(0));
        c.exec("-V").expect_status_code(Some(0));
    }
    {
        let recipe = s::Build001::example_with_steps();
        c.write_json("example1.json", &recipe);
        c.create_blank_image_here("200x200", 200, 200, s::EncoderPreset::libjpeg_turbo());
        c.create_blank_image_here("200x200", 200, 200, s::EncoderPreset::libpng32());

        c.exec("v0.1/build --json example1.json --in 200x200.png 200x200.jpg --out out0.jpg --response out0.json").expect_exit_0_no_output("");
        // TODO: Verify out0.json exists and was created
        c.exec("v0.1/build --bundle-to bundle_example_1 --json example1.json --in 200x200.png 200x200.jpg --out out0.jpg --response out0.json").expect_status_code(Some(0));
        //TODO: verify bundle was created
        //TODO: test URL fetch
    }
    {
        let recipe =  s::Build001::example_with_steps();
        c.write_json("example2.json",&recipe);
        c.create_blank_image_here("200x200", 200, 200, s::EncoderPreset::libjpeg_turbo());
        c.create_blank_image_here("200x200", 200, 200, s::EncoderPreset::libpng32());

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
        let recipe = fluent::fluently().decode(0).constrain_within(Some(60), Some(45), None).encode(1, s::EncoderPreset::libjpeg_turbo()).into_build_0_1();
        c.write_json("example2.json", &recipe);
        c.create_blank_image_here("200x200", 200, 200, s::EncoderPreset::libjpeg_turbo());

        let result =
        c.exec("v0.1/build --json example2.json --in 200x200.jpg --out out3.jpg");

        result.expect_status_code(Some(0));

        let resp: s::Response001 = result.parse_stdout_as::<s::Response001>().unwrap();
        match resp.data {
            s::ResponsePayload::BuildResult(info) => {

                assert_eq!(info.encodes.len(), 1);
                let encode: &s::EncodeResult = &info.encodes[0];
                assert_eq!(encode.w, 45);
                assert_eq!(encode.h, 45);
            }
            _ => panic!("Build result not sent"),
        }

    }
    {
        let c = c.subfolder_context("query");
        c.create_blank_image_here("100x100", 100, 100, s::EncoderPreset::libjpeg_turbo());

        let result =
            c.exec("v0.1/ir4 --command width=60&height=40&mode=max&format=jpg --in 100x100.jpg --out out4.jpg");

        result.expect_status_code(Some(0));

        let resp: s::Response001 = result.parse_stdout_as::<s::Response001>().unwrap();
        match resp.data {
            s::ResponsePayload::BuildResult(info) => {

                assert_eq!(info.encodes.len(),1);
                let encode: &s::EncodeResult = &info.encodes[0];
                assert_eq!(encode.w, 40);
                assert_eq!(encode.h, 40);
            }
            _ => panic!("Build result not sent"),
        }

    }
    {
        let c = c.subfolder_context("queryquiet");
        c.create_blank_image_here("100x100", 100, 100, s::EncoderPreset::libjpeg_turbo());

        let result =
            c.exec("v0.1/ir4 --quiet --command \"width=60&height=40&mode=max&format=jpg\" --in 100x100.jpg --out out4.jpg");

        result.expect_status_code(Some(0));
        assert_eq!(0, result.stdout_byte_count());

    }
    {
        let c = c.subfolder_context("gif");
        let result =
            c.exec("v0.1/ir4 --command width=200&height=200&format=gif --in https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg --out out5.gif");

        result.expect_status_code(Some(0));

//        let resp: s::Response001 = result.parse_stdout_as::<s::Response001>().unwrap();
//        match resp.data {
//            s::ResponsePayload::BuildResult(info) => {
//
//                assert_eq!(info.encodes.len(), 1);
//                let encode: &s::EncodeResult = &info.encodes[0];
//                assert_eq!(encode.preferred_extension, "gif".to_owned());
//            }
//            _ => panic!("Build result not sent"),
//        }


    }

    // It seems that Clap always uses status code 1 to indicate a parsing failure
    c.exec("bad command").expect_status_code(Some(1));

    // Write something unexpected, but valid JSON
    c.write_json("random_object.json", &s::PngBitDepth::Png24);

    c.exec("v0.1/build --json random_object.json")
        .expect_status_code(Some(65))
        .expect_stderr_contains("expected struct Build001");
    // .expect_stdout_contains("")   ; //todo: should respond with JSON version of error message

    {
        // Test having both input and canvas point to the same bitmap
        // This should fail
        // TODO: THis should fail in a consistent way, as a bad parameter situation
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

pub fn test_capture(tool_location: Option<PathBuf>) -> i32 {
    let c = ProcTestContext::create_timestamp_subdir_within(std::env::current_dir().unwrap().join("self_tests"), tool_location);
    // encapsulate scenario/example for reuse
    {
        let recipe = s::Build001::example_with_steps();
        c.write_json("example1.json", &recipe);
        c.create_blank_image_here("200x200", 200, 200, s::EncoderPreset::libjpeg_turbo());
        c.create_blank_image_here("200x200", 200, 200, s::EncoderPreset::libpng32());

        c.exec("v0.1/build --json example1.json --in 200x200.png 200x200.jpg --out out0.jpg --response out0.json").expect_exit_0_no_output("");
        // TODO: Verify out0.json exists and was created
        c.exec("v0.1/build --bundle-to bundle_example_1 --json example1.json --in 200x200.png 200x200.jpg --out out0.jpg --response out0.json").expect_status_code(Some(0));
        //TODO: verify bundle was created
        //TODO: test URL fetch
        c.subfolder_context(Path::new("bundle_example_1")).exec("--capture-to recipe v0.1/build --json recipe.json --response response.json").dump().expect_status_code(Some(0));

    }
    {
        let recipe = s::Build001::example_with_steps();
        c.write_json("example1.json", &recipe);
        c.create_blank_image_here("200x200", 200, 200, s::EncoderPreset::libjpeg_turbo());
        c.create_blank_image_here("200x200", 200, 200, s::EncoderPreset::libpng32());

        c.exec("v0.1/build --debug-package debug_example --json example1.json --in 200x200.png 200x200.jpg --out out0.jpg --response out0.json").expect_status_code(Some(0));
    }




    0
}
