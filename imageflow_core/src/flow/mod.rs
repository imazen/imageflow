use ::internal_prelude::external::*;

#[macro_use]
pub mod definitions;
pub mod execution_engine;
pub mod visualize;
pub mod nodes;
use self::definitions::*;


//
//#[macro_export]
//macro_rules! error_return (
//    ($context:expr) => (
//        unsafe {
//            flow_context_add_to_callstack($context, concat!(file!(), "\0").as_ptr() as *const libc::c_char,
//                line!() as i32, concat!(module_path!(), "\0").as_ptr() as *const libc::c_char);
//        }
//    );
//);
//
//#[macro_export]
//macro_rules! error_msg (
//    ($context:expr, $status: expr) => (
//        unsafe {
//            let c = CStr::from_ptr($crate::ffi::flow_context_set_error_get_message_buffer($context, $status as i32,
//                concat!(file!(), "\0").as_ptr() as *const libc::c_char,
//                line!() as i32, concat!(module_path!(), "\0").as_ptr() as *const libc::c_char));
//            println!("{:?}", c);
//        }
//    );
//    ($context:expr, $status: expr, $format:expr, $($args:expr),*) => (
//        let c = CStr::from_ptr($crate::ffi::flow_context_set_error_get_message_buffer($context, $status as i32,
//            concat!(file!(), "\0").as_ptr() as *const libc::c_char,
//            line!() as i32, concat!(module_path!(), "\0").as_ptr() as *const libc::c_char));
//        let formatted = fmt::format(format_args!(concat!($format, "\0"),$($args),*));
//        println!("{:?} {}", c, formatted);
//    );
//);
