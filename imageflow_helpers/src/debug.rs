use crate::preludes::from_std::*;
use std;

use backtrace::{Backtrace, BacktraceFrame};
use std::panic;
use std::thread;

use std::sync::{Once};

static CONDITIONAL_SET: Once = Once::new();
static SET_HOOK: Once = Once::new();


///
/// Returns true if `RUST_BACKTRACE=1`
pub fn backtraces_wanted() -> bool{
    if let Ok(val) = std::env::var("RUST_BACKTRACE"){
        val == "1"
    }else{
        false
    }
}
pub fn upgrade_panic_hook_once_if_backtraces_wanted(){
    CONDITIONAL_SET.call_once(|| {
        if backtraces_wanted(){
            set_panic_hook_once();
        }
    });
}

/// Only executes the first time it is called
pub fn set_panic_hook_once() {
    SET_HOOK.call_once(|| {
        set_panic_hook();
    });
}

fn set_panic_hook(){
    panic::set_hook(Box::new(|info| {

        let thread = thread::current();
        let thread = thread.name().unwrap_or("unnamed");

        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &**s,
                None => "Box<Any>",
            }
        };

        match info.location() {
            Some(location) => {
                let _ = writeln!(&mut std::io::stderr(), "thread '{}' panicked at '{}': \n{}:{}:",
                thread,
                msg,
                location.file(),
                location.line());
            }
            None => {
                let _ = writeln!(&mut std::io::stderr(), "thread '{}' panicked at '{}'", thread, msg);
            },
        }


        //std::panicking::begin_panic
        // TODO: customize from https://github.com/alexcrichton/backtrace-rs/blob/master/src/capture.rs
        // Consider timeout on resolving frames
        // Consider partial printing (lazy eval for console)
        // Skip if OOM


        let mut frames: Vec<BacktraceFrame> = Backtrace::new().into();

        loop{
            if frames.len() < 1 || frames[0].symbols().is_empty() || frames[0].symbols()[0].name().is_none(){
                break;
            }

            let last_pointless_frame = if let Some(str) = frames[0].symbols()[0].name().unwrap().as_str(){
                str.starts_with("std::panicking::begin_panic")
            } else {false};

            frames.remove(0);

            if last_pointless_frame{
                break;
            }
        }

        let _ = writeln!(&mut std::io::stderr(), "{:?}", &Backtrace::from(frames));

    }));

}

