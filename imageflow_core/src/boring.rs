// Boring, because we're not doing any kind of op graph, just a static list of configurable ops.

use ffi::*;
use flow;
use std::ffi::*;
use std::fs::File;
use std::io::Read;
use std::mem;
use std::ptr;
extern crate libc;
extern crate threadpool;
extern crate serde;
extern crate serde_json;
extern crate time;
use std::path::PathBuf;
use std::str::FromStr;
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

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "jpeg" => Ok(ImageFormat::Jpeg),
            "jpg" => Ok(ImageFormat::Jpeg),
            "png" => Ok(ImageFormat::Png),
            "png24" => Ok(ImageFormat::Png24),
            _ => Err("no match"),
        }
    }
}


impl FromStr for ConstraintMode {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
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
    pub w: i32,
    pub h: i32,
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
                              -> Result<(), String> {


    let c_input_path = CString::new(input_path.to_str().unwrap().as_bytes()).unwrap().into_raw();

    let c_output_path = CString::new(output_path.to_str().unwrap().as_bytes()).unwrap().into_raw();

    let result = process_image(commands,
                               |c| {

        unsafe {



            let input_io = flow_io_create_for_file(c,
                                                   IoMode::read_seekable,
                                                   c_input_path,
                                                   c as *mut libc::c_void);

            // TODO! Lots of error handling needed here. IO create/add can fail
            if input_io.is_null() {
                flow_context_print_and_exit_if_err(c);
            }
            let output_io = flow_io_create_for_file(c,
                                                    IoMode::write_seekable,
                                                    c_output_path,
                                                    c as *mut libc::c_void);
            if output_io.is_null() {
                flow_context_print_and_exit_if_err(c);
            }


            vec![IoResource {
                     io: input_io,
                     direction: IoDirection::In,
                 },
                 IoResource {
                     io: output_io,
                     direction: IoDirection::Out,
                 }]
        }
    },
                               |_, _| Ok(()));

    // Bring the paths back into Rust ownership so they can be collected.
    unsafe {
        CString::from_raw(c_input_path);
        CString::from_raw(c_output_path);
    }
    return result;
}

pub struct BenchmarkOptions {
    pub input_path: PathBuf,
    pub commands: BoringCommands,
    pub thread_count: usize,
    pub run_count: usize,
}

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    result: Result<(), String>,
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
                       serde_json::Value::I64(i.wall_nanoseconds));
            let ms_str = format!("{:?}ms",
                                 time::Duration::nanoseconds(i.wall_nanoseconds)
                                     .num_milliseconds());

            map.insert(String::from("wall_ms"), serde_json::Value::String(ms_str));

            list.push(serde_json::Value::Object(map));
        }

        let mut root = serde_json::Map::new();
        root.insert("runs", serde_json::Value::Array(list));
        root.insert("wall_ns", serde_json::Value::I64(self.wall_nanoseconds));

        let ms_str = format!("{:?}ms",
                             time::Duration::nanoseconds(self.wall_nanoseconds).num_milliseconds());

        let avg_str = format!("{:?}ms",
                              time::Duration::nanoseconds(self.wall_nanoseconds /
                                                          (self.count as i64))
                                  .num_milliseconds());

        root.insert("wall_ms", serde_json::Value::String(ms_str));

        root.insert("avg_ms", serde_json::Value::String(avg_str));
        return serde_json::to_string(&root).unwrap();

    }
}



fn benchmark_op(cmds: BoringCommands, mem: *mut u8, len: usize) -> BenchmarkResult {
    let begin_at = time::precise_time_ns();
    let result = process_image(cmds,
                               |c| {
        unsafe {
            let input_io = flow_io_create_from_memory(c,
                                                      IoMode::read_seekable,
                                                      mem,
                                                      len,
                                                      c as *mut libc::c_void,
                                                      ptr::null());

            if input_io.is_null() {
                flow_context_print_and_exit_if_err(c);
            }
            let output_io = flow_io_create_for_output_buffer(c, c as *mut libc::c_void);
            if output_io.is_null() {
                flow_context_print_and_exit_if_err(c);
            }

            vec![IoResource {
                     io: input_io,
                     direction: IoDirection::In,
                 },
                 IoResource {
                     io: output_io,
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
pub fn benchmark(bench: BenchmarkOptions) -> Result<BenchmarkResults, String> {

    let mut f = File::open(bench.input_path).unwrap();//bad
    let mut buffer = Vec::new();

    // read the whole file
    f.read_to_end(&mut buffer).unwrap();//bad


    let len = buffer.len();

    let mem = buffer.as_mut_ptr();

    let cap = buffer.capacity();


    mem::forget(buffer);

    let pool = threadpool::ThreadPool::new(bench.thread_count);

    let (tx, rx) = channel();



    let begin_at = time::precise_time_ns();

    for _ in 0..bench.run_count {
        let tx = tx.clone();
        let m = mem.clone() as i64;
        let l = len.clone();
        let cmds = bench.commands.clone();
        pool.execute(move || {
            tx.send(benchmark_op(cmds, m as *mut u8, l)).unwrap();
        });
    }

    let mut res_list = Vec::new();
    let result_iterator = rx.iter().take(bench.run_count as usize);
    for i in result_iterator {
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

    return Ok(BenchmarkResults {
        list: res_list,
        wall_nanoseconds: (end_at - begin_at) as i64,
        threads: bench.thread_count,
        count: bench.run_count,
    });
}


pub struct IoResource {
    pub io: *mut JobIO,
    pub direction: IoDirection,
}

pub fn process_image<F, C, R>(commands: BoringCommands,
                              io_provider: F,
                              cleanup: C)
                              -> Result<R, String>
    where F: Fn(*mut Context) -> Vec<IoResource>,
          C: Fn(*mut Context, *mut Job) -> Result<R, String>
{
    unsafe {

        let c = flow_context_create();
        assert!(!c.is_null());

        if commands.luma_correct {
            flow_context_set_floatspace(c, Floatspace::linear, 0f32, 0f32, 0f32);
        } else {
            flow_context_set_floatspace(c, Floatspace::srgb, 0f32, 0f32, 0f32);
        }


        let j = flow_job_create(c);
        assert!(!j.is_null());


        let mut inputs = io_provider(c);


        for (index, input) in inputs.iter_mut().enumerate() {
            // TODO! Lots of error handling needed here. IO create/add can fail
            if !flow_job_add_io(c, j, input.io, index as i32, input.direction.clone()) {
                flow_context_print_and_exit_if_err(c);
            }

        }

        let mut info = DecoderInfo { ..Default::default() };

        if !flow_job_get_decoder_info(c, j, 0, &mut info) {
            flow_context_print_and_exit_if_err(c);
        }


        let constraint_ratio = (commands.w as f32) / (commands.h as f32);
        let natural_ratio = (info.frame0_width as f32) / (info.frame0_height as f32);
        let final_w;
        let final_h;

        match commands.fit {
            ConstraintMode::Max => {
                if info.frame0_width > commands.w || info.frame0_height > commands.h {
                    if constraint_ratio > natural_ratio {
                        final_h = commands.h as usize;
                        final_w = (commands.h as f32 * natural_ratio).round() as usize;
                    } else {
                        final_w = commands.w as usize;
                        final_h = (commands.w as f32 / natural_ratio).round() as usize;
                    }
                } else {
                    final_w = info.frame0_width as usize;
                    final_h = info.frame0_height as usize;
                }
            }
            ConstraintMode::Distort => {
                final_h = commands.h as usize;
                final_w = commands.w as usize;
            }
        };

        let trigger_ratio = if 1.0f32 > commands.precise_scaling_ratio {
            3.0f32
        } else {
            commands.precise_scaling_ratio
        };


        let pre_w = ((final_w as f32) * trigger_ratio).round() as i64;
        let pre_h = ((final_h as f32) * trigger_ratio).round() as i64;
        if !flow_job_decoder_set_downscale_hints_by_placeholder_id(c,
                                                                   j,
                                                                   0,
                                                                   pre_w,
                                                                   pre_h,
                                                                   pre_w,
                                                                   pre_h,
                                                                   commands.luma_correct,
                                                                   commands.luma_correct) {
            flow_context_print_and_exit_if_err(c);
        }


        // println!("Scale {}x{} down to {}x{} (jpeg)", info.frame0_width, info.frame0_height, final_w, final_h);

        //TODO: Replace with s::Node, s::Graph, etc.

        let mut g = flow_graph_create(c, 10, 10, 10, 2.0);
        assert!(!g.is_null());


        let mut last = flow_node_create_decoder(c, (&mut g) as *mut *mut Graph, -1, 0);
        assert!(last == 0);

        last = flow_node_create_scale(c,
                                      (&mut g) as *mut *mut Graph,
                                      last,
                                      final_w,
                                      final_h,
                                      commands.down_filter as i32,
                                      commands.up_filter as i32,
                                      1,
                                      commands.sharpen);

        assert!(last > 0);

        let disable_png_alpha = match commands.format {
            ImageFormat::Png24 => true,
            _ => false,
        };

        let hints = EncoderHints {
            jpeg_quality: commands.jpeg_quality,
            disable_png_alpha: disable_png_alpha,
        };

        let encoder_id = match commands.format {
            ImageFormat::Png24 => ImageFormat::Png,
            f => f,

        } as i64;


        last =
            flow_node_create_encoder(c, (&mut g) as *mut *mut Graph, last, 1, encoder_id, &hints);
        assert!(last > 0);


        if !flow::job_execute(c, j, (&mut g) as *mut *mut Graph) {
            flow_context_print_and_exit_if_err(c);
        }

        let result = cleanup(c, j);
        // TODO: call both cleanup functions and print errors

        if !flow_context_begin_terminate(c) {
            flow_context_print_and_exit_if_err(c);
        }

        flow_context_destroy(c);

        return result;
    }
}
