#![feature(alloc_system)]

// This executable is for valgrind execution when something goes wrong
extern crate libc;
extern crate alloc_system;
extern crate imageflow_core;

fn main() {
    for _ in 0..50 {
        ::imageflow_core::abi::exercise_error_handling();
        ::imageflow_core::abi::exercise_json_message();
        ::imageflow_core::abi::exercise_json_message();
        ::imageflow_core::abi::exercise_error_handling();
    }
}
