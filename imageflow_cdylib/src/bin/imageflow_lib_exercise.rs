#![feature(alloc_system)]

// This executable is for valgrind execution when something goes wrong
// You need to set crate_type = ["rlib", "cdylib"] to let this compile
extern crate libc;
extern crate alloc_system;
// extern crate imageflowrs;

fn main() {
  println!("This executable has been disabled; see source code.")
  // for _ in 0..50 {
  //   ::imageflowrs::exercise_error_handling();
  //   ::imageflowrs::exercise_json_message();
  //   ::imageflowrs::exercise_json_message();
  //   ::imageflowrs::exercise_error_handling();
  // }
}
