use ffi::*;
use std::ffi::*;
extern crate libc;
use std::path::PathBuf;

pub enum ConstraintMode {
    Max,
}

pub struct BoringCommands {
    pub fit: ConstraintMode,
    pub w: i32,
    pub h: i32,
    pub precise_scaling_ratio: f32,
    pub luma_correct: bool,
    pub jpeg_quality: i32,
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
        if constraint_ratio > natural_ratio {
            final_h = commands.h as usize;
            final_w = (commands.h as f32 * natural_ratio).round() as usize;
        } else {
            final_w = commands.w as usize;
            final_h = (commands.w as f32 / natural_ratio).round() as usize;
        }

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

        let mut g = flow_graph_create(c, 10, 10, 10, 2.0);
        assert!(!g.is_null());


        let mut last = flow_node_create_decoder(c, (&mut g) as *mut *mut Graph, -1, 0);
        assert!(last == 0);

        last = flow_node_create_scale(c,
                                      (&mut g) as *mut *mut Graph,
                                      last,
                                      final_w,
                                      final_h,
                                      Filter::Robidoux,
                                      Filter::Robidoux,
                                      0);

        assert!(last > 0);

        let hints = EncoderHints { jpeg_quality: commands.jpeg_quality };

        last = flow_node_create_encoder(c, (&mut g) as *mut *mut Graph, last, 1, 4, &hints);
        assert!(last > 0);


        if !flow_job_execute(c, j, (&mut g) as *mut *mut Graph) {
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
