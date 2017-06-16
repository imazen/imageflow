// Boring, because we're not doing any kind of op graph, just a static list of configurable ops.

use ::internal_prelude::works_everywhere::*;
extern crate threadpool;
use Context;
use Job;
use JsonResponse;
use IoProxy;
use ::ffi::*;
use std::sync::mpsc::channel;

#[derive(Copy,Clone, Debug)]
pub enum ConstraintMode {
    Max,
    Distort,
}

#[derive(Copy,Clone, Debug)]
pub enum ImageFormat {
    Jpeg = 4,
    Png = 2,
    Png24 = 9,
}


impl FromStr for ImageFormat {
    type Err = &'static str;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "jpeg" | "jpg" => Ok(ImageFormat::Jpeg),
            "png" => Ok(ImageFormat::Png),
            "png24" => Ok(ImageFormat::Png24),
            _ => Err("no match"),
        }
    }
}


impl FromStr for ConstraintMode {
    type Err = &'static str;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "max" => Ok(ConstraintMode::Max),
            "distort" => Ok(ConstraintMode::Distort),
            _ => Err("no match"),
        }
    }
}

#[derive(Copy,Clone, Debug)]
pub struct BoringCommands {
    pub fit: ConstraintMode,
    pub w: Option<i32>,
    pub h: Option<i32>,
    pub precise_scaling_ratio: f32,
    pub luma_correct: bool,
    pub jpeg_quality: i32,
    pub format: ImageFormat,
    pub sharpen: f32,
    pub down_filter: Filter,
    pub up_filter: Filter,
}


pub fn process_image_by_paths(input_path: PathBuf,
                              output_path: PathBuf,
                              commands: BoringCommands)
                              -> std::result::Result<(), String> {
    process_image(commands,
                  |c: &Context| {
                      let input_io =
                      c.create_io_from_filename(input_path.as_path().as_os_str().to_str().unwrap(), ::IoDirection::In).unwrap();
                      let output_io = c.create_io_from_filename(output_path.as_path().as_os_str().to_str().unwrap(), ::IoDirection::Out).unwrap();


                      vec![IoResource {
                          io: input_io,
                          direction: IoDirection::In,
                      },
                      IoResource {
                          io: output_io,
                          direction: IoDirection::Out,
                      }]
                  },
                  |_, _| Ok(()))
}

pub struct BenchmarkOptions {
    pub input_path: PathBuf,
    pub commands: BoringCommands,
    pub thread_count: usize,
    pub run_count: usize,
}

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    result: std::result::Result<(), String>,
    wall_nanoseconds: i64,
}
#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    list: Vec<BenchmarkResult>,
    wall_nanoseconds: i64,
    threads: usize,
    count: usize,
}


impl BenchmarkResults {
    pub fn to_json_string(&self) -> String {

        let mut list = Vec::new();
        for i in &self.list {

            let mut map = serde_json::Map::new();
            map.insert(String::from("ns"),
                       serde_json::Value::Number(serde_json::Number::from(i.wall_nanoseconds)));
            let ms_str = format!("{:?}ms",
                                 time::Duration::nanoseconds(i.wall_nanoseconds)
                                     .num_milliseconds());

            map.insert(String::from("wall_ms"), serde_json::Value::String(ms_str));

            list.push(serde_json::Value::Object(map));
        }

        let mut root = serde_json::Map::new();
        root.insert("runs".to_owned(), serde_json::Value::Array(list));
        root.insert("wall_ns".to_owned(), serde_json::Value::Number(serde_json::Number::from(self.wall_nanoseconds)));

        let ms_str = format!("{:?}ms",
                             time::Duration::nanoseconds(self.wall_nanoseconds).num_milliseconds());

        let avg_str = format!("{:?}ms",
                              time::Duration::nanoseconds(self.wall_nanoseconds /
                                                          (self.count as i64))
                                  .num_milliseconds());

        root.insert("wall_ms".to_owned(), serde_json::Value::String(ms_str));

        root.insert("avg_ms".to_owned(), serde_json::Value::String(avg_str));
        serde_json::to_string(&root).unwrap()

    }
}



fn benchmark_op(cmds: BoringCommands, mem: *mut u8, len: usize) -> BenchmarkResult {
    let begin_at = time::precise_time_ns();
    let result = process_image(cmds,
                               |c: &Context| {
        unsafe {
            vec![IoResource {
                     io: c.create_io_from_slice(std::slice::from_raw_parts(mem, len)).unwrap(),
                     direction: IoDirection::In,
                 },
                 IoResource {
                     io: c.create_io_output_buffer().unwrap(),
                     direction: IoDirection::Out,
                 }]
        }
    },
                               |_, _| Ok(()));
    let end_at = time::precise_time_ns();
    BenchmarkResult {
        result: result,
        wall_nanoseconds: (end_at - begin_at) as i64,
    }
}
pub fn benchmark(bench: BenchmarkOptions) -> std::result::Result<BenchmarkResults, String> {

    // Switch to Arc instead of pointers
    let mut f = File::open(bench.input_path).unwrap();
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer).unwrap();

    let len = buffer.len();
    let mem = buffer.as_mut_ptr();
    let cap = buffer.capacity();

    mem::forget(buffer);

    let pool = threadpool::ThreadPool::new(bench.thread_count);
    let (tx, rx) = channel();

    let begin_at = time::precise_time_ns();

    for _ in 0..bench.run_count {
        let tx = tx.clone();
        let m = mem as i64;
        let l = len;
        let cmds = bench.commands;
        pool.execute(move || {
            tx.send(benchmark_op(cmds, m as *mut u8, l)).unwrap();
        });
    }

    let mut res_list = Vec::new();
    let result_iterator = rx.iter().take(bench.run_count as usize);
    for i in result_iterator {
        // res_list.push(i.result?)
        match i.result {
            Ok(_) => {
                res_list.push(i);
            }
            Err(e) => {
                return Err(e);
            }
        }
    }
    let end_at = time::precise_time_ns();

    unsafe {
        let _ = Vec::from_raw_parts(mem, len, cap);
    }

    Ok(BenchmarkResults {
        list: res_list,
        wall_nanoseconds: (end_at - begin_at) as i64,
        threads: bench.thread_count,
        count: bench.run_count,
    })
}


pub struct IoResource<'a> {
    pub io: RefMut<'a, IoProxy>,
    pub direction: IoDirection,
}

fn constrain(original_width: i32,
             original_height: i32,
             constrain: ConstraintMode,
             constrain_w: Option<i32>,
             constrain_h: Option<i32>)
             -> (usize, usize) {

    let natural_ratio = (original_width as f32) / (original_height as f32);
    let final_w;
    let final_h;

    // println!("{:?}", commands);
    if constrain_h.is_none() && constrain_w.is_none() {
        final_w = original_width as usize;
        final_h = original_height as usize;
    } else {
        let w = match constrain_w {
            Some(w) => w,
            None => (constrain_h.unwrap() as f32 * natural_ratio).round() as i32,
        };
        let h = match constrain_h {
            Some(h) => h,
            None => (constrain_w.unwrap() as f32 / natural_ratio).round() as i32,
        };

        match constrain {
            ConstraintMode::Max => {
                if original_width > w || original_height > h {
                    let constraint_ratio = (w as f32) / (h as f32);
                    if constraint_ratio > natural_ratio {
                        final_h = h as usize;
                        final_w = (h as f32 * natural_ratio).round() as usize;
                    } else {
                        final_w = w as usize;
                        final_h = (w as f32 / natural_ratio).round() as usize;
                    }
                } else {
                    final_w = original_width as usize;
                    final_h = original_height as usize;
                }
            }
            ConstraintMode::Distort => {
                final_h = h as usize;
                final_w = w as usize;
            }
        };
    }
    (final_w, final_h)
}

#[test]
fn test_constraining() {
    assert_eq!((100, 50),
               constrain(200, 100, ConstraintMode::Max, Some(100), None));
    assert_eq!((400, 200),
               constrain(200, 100, ConstraintMode::Distort, Some(400), None));
}

pub fn create_framewise(original_width: i32,
                        original_height: i32,
                        commands: BoringCommands)
                        -> std::result::Result<(s::Framewise, (i32, i32)), String> {
    let (final_w, final_h) = constrain(original_width,
                                       original_height,
                                       commands.fit,
                                       commands.w,
                                       commands.h);

    // Should we IDCT downscale?

    let trigger_ratio = if 1.0f32 > commands.precise_scaling_ratio {
        3.0f32
    } else {
        commands.precise_scaling_ratio
    };

    let pre_w = ((final_w as f32) * trigger_ratio).round() as i32;
    let pre_h = ((final_h as f32) * trigger_ratio).round() as i32;


    let encoder_preset = match commands.format {
        ImageFormat::Jpeg => {
            s::EncoderPreset::LibjpegTurbo { quality: Some(commands.jpeg_quality) }
        }
        ImageFormat::Png => {
            s::EncoderPreset::Libpng {
                zlib_compression: None,
                matte: None,
                depth: Some(s::PngBitDepth::Png32),
            }
        }
        ImageFormat::Png24 => {
            s::EncoderPreset::Libpng {
                zlib_compression: None,
                matte: Some(s::Color::Black),
                depth: Some(s::PngBitDepth::Png24),
            }
        }
    };

    let steps = vec![s::Node::Decode {
                 io_id: 0,
                 commands:
                     Some(vec![s::DecoderCommand::JpegDownscaleHints(s::JpegIDCTDownscaleHints {
                                   width: pre_w as i64,
                                   height: pre_h as i64,
                                   scale_luma_spatially: Some(commands.luma_correct),
                                   gamma_correct_for_srgb_during_spatial_luma_scaling:
                                       Some(commands.luma_correct),
                               })]),
             },
             s::Node::Resample2D {
                 w: final_w,
                 h: final_h,
                 down_filter: Some(commands.down_filter),
                 up_filter: Some(commands.up_filter),
                 hints: Some(s::ResampleHints {
                     sharpen_percent: Some(commands.sharpen),
                     prefer_1d_twice: None,
                 }),
             },
             s::Node::Encode {
                 io_id: 1,
                 preset: encoder_preset,
             }];
    Ok((s::Framewise::Steps(steps), (pre_w, pre_h)))
}


pub fn process_image<F, C, R>(commands: BoringCommands,
                              io_provider: F,
                              cleanup: C)
                              -> std::result::Result<R, String>
    where F: Fn(&Context) -> Vec<IoResource>,
          C: Fn(&Context, &Job) -> std::result::Result<R, String>
{
    let context = Context::create().unwrap();
    let result;

    {
        let mut job = context.create_job();

        // Add I/O
        {
            let mut inputs: Vec<IoResource> = io_provider(&context);
            for (index, input) in inputs.iter_mut().enumerate() {
                job.add_io(&mut input.io, index as i32, input.direction).unwrap();
            }
        }


        let info_blob: JsonResponse =
        job.message("v0.1/get_image_info", b"{\"io_id\": 0}").unwrap();
        let info_response: s::Response001 =
        serde_json::from_slice(info_blob.response_json.as_ref()).unwrap();
        if !info_response.success {
            panic!("get_image_info failed: {:?}", info_response);
        }
        let (image_width, image_height) = match info_response.data {
            s::ResponsePayload::ImageInfo(info) => (info.image_width, info.image_height),
            _ => panic!(""),
        };


        let (framewise, (pre_w, pre_h)) = create_framewise(image_width, image_height, commands)
            .unwrap();

        if pre_w < image_width && pre_h < image_height {
            let send_hints = s::TellDecoder001 {
                io_id: 0,
                command: s::DecoderCommand::JpegDownscaleHints(s::JpegIDCTDownscaleHints {
                    height: pre_h as i64,
                    width: pre_w as i64,
                    scale_luma_spatially: Some(commands.luma_correct),
                    gamma_correct_for_srgb_during_spatial_luma_scaling: Some(commands.luma_correct),
                }),
            };
            let send_hints_str = serde_json::to_string_pretty(&send_hints).unwrap();
            job.message("v0.1/tell_decoder", send_hints_str.as_bytes()).unwrap().assert_ok();
        }


        let send_execute = s::Execute001 {
            framewise: framewise,
            graph_recording: None,
            no_gamma_correction: Some(!commands.luma_correct),
        };

        let send_execute_str = serde_json::to_string_pretty(&send_execute).unwrap();
        job.message("v0.1/execute", send_execute_str.as_bytes()).unwrap().assert_ok();


        result = cleanup(&context, &*job);
    }

    context.destroy_allowing_panics();
    result
}
