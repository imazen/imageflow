extern crate imageflow_c_components;
extern crate libc;
use libc::{size_t,c_schar,c_char};
use std::{file,line};

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ImageflowContext {
    pub error: ErrorInfo,
    pub underlying_heap: Heap,
    pub log: ProfilingLog,
    pub object_tracking: ObjTrackingInfo,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
enum ImageFlowStatusCode {
    flow_status_No_Error = 0,
    flow_status_Out_of_memory = 10,
    flow_status_IO_error = 20,
    flow_status_Invalid_internal_state = 30,
    flow_status_Panic = 31,
    flow_status_Not_implemented = 40,
    flow_status_Invalid_argument = 50,
    flow_status_Null_argument = 51,
    flow_status_Invalid_dimensions = 52,
    flow_status_Unsupported_pixel_format = 53,
    flow_status_Item_does_not_exist = 54,

    flow_status_Image_decoding_failed = 60,
    flow_status_Image_encoding_failed = 61,
    flow_status_ErrorReportingInconsistency = 90,
    flow_status_First_rust_error = 200,

    flow_status_Other_error = 1024,
    flow_status_First_user_defined_error = 1025,
    flow_status_Last_user_defined_error = 2147483647
}

macro_rules! set_image_flow_error {
    ($context:ident,$code:ident,$name:ident) => {
    {
    flow_context_set_error_get_message_buffer($context,$code,file!(),!line(),$name)
    }
    };
}

extern "C" {
    pub fn flow_context_create() -> *mut ImageflowContext;
    pub fn flow_sanity_check(check:*mut SanityCheck);
    pub fn flow_snprintf(buf:*mut c_char,size:size_t,format: *const c_schar)->i32;
    pub fn flow_context_destroy(context:*mut ImageflowContext);
    pub fn flow_context_set_error_get_message_buffer(context:*ImageflowContext,code:ImageflowContext , file:c_schar, line:i32,
                                                     function_name:c_schar)->*const c_schar;
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct SanityCheck{
    sizeof_bool:u32,
    sizeof_int:u32,
    sizeof_size_t:usize,
}

/** flow context: Heap Manager **/
#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct Heap {
    placeholder: u8,
}
#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ErrorInfo {
    placeholder: u8,
}


#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct HeapObjectRecord {
    placeholder: u8,
}

#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ObjTrackingInfo {
    pub allocs: HeapObjectRecord,
    pub next_free_slot: size_t,
    pub total_slots: size_t,
    pub bytes_allocated_net: size_t,
    pub bytes_allocated_gross: size_t,
    pub allocations_net: size_t,
    pub allocations_gross: size_t,
    pub bytes_free: size_t,
    pub allocations_net_peak: size_t,
    pub bytes_allocations_net_peak: size_t,
}


#[repr(C)]
#[derive(Clone,Debug,PartialEq)]
pub struct ProfilingLog {
    placeholder: u8, // FIXME: replace
}


#[cfg(test)]
mod context_test {
    use ::{flow_sanity_check, SanityCheck,flow_context_create,flow_snprintf};
    use std::mem::size_of;
    use ::{size_t, ImageflowContext,c_char};
    use std::ffi::CString;
    use flow_context_destroy;

    #[test]
    fn test_create_context() {
        unsafe{
            let v=flow_context_create();
            assert_eq!(v.is_null(),false);
            flow_context_destroy(v);
        }
    }

    #[test]
    fn sanity_check(){
        unsafe{
            let mut check=std::mem::MaybeUninit::new(std::mem::zeroed::<SanityCheck>());
            flow_sanity_check(check.as_mut_ptr());
            assert_eq!((*check.as_ptr()).sizeof_bool as usize,(size_of::<bool>()));
            assert_eq!((*check.as_ptr()).sizeof_int as usize,(size_of::<i32>()));
            assert_eq!((*check.as_ptr()).sizeof_size_t as usize,(size_of::<size_t>()));
        }
    }

    #[test]
    fn test_imageflow_context_size(){

            assert_eq!(size_of::<ImageflowContext>()<1500,true);
    }

    #[test]
    fn test_flow_flow_snprintf_single_char(){
        unsafe {
            let mut buf:[c_char;2]=[3,25];
            let raw_buf=buf.as_mut_ptr();
            assert_eq!(flow_snprintf(raw_buf,1,CString::new("hello").unwrap().as_ptr()),-1);
            assert_eq!(buf[0],0);
            assert_eq!(buf[1],25);
        }
    }

    #[test]
    fn test_flow_flow_snprintf_zero_char(){
        unsafe {
            let mut buf:[c_char;2]=[3,25];
            let raw_buf=buf.as_mut_ptr();
            assert_eq!(flow_snprintf(raw_buf,0,CString::new("hello").unwrap().as_ptr()),-1);
            assert_eq!(buf[0],3);
            assert_eq!(buf[1],25);
        }
    }


    #[test]
    fn test_flow_flow_snprintf_insufficeint_buffer(){
        unsafe {
            let mut buf:[c_char;4]= [23, 27, 25, 26 ];
            let raw_buf=buf.as_mut_ptr();
            assert_eq!(flow_snprintf(raw_buf,3,CString::new("hello").unwrap().as_ptr()),-1);
            assert_eq!(buf[0],104);
            assert_eq!(buf[1],101);
            assert_eq!(buf[2],0);
            assert_eq!(buf[3], 26);
        }
    }


    #[test]
    fn test_flow_flow_snprintf_sufficeint_buffer(){
        unsafe {
            let mut buf:[c_char;7]= [23, 27, 25, 26,38,09,89 ];
            let raw_buf=buf.as_mut_ptr();
            assert_eq!(flow_snprintf(raw_buf,6,CString::new("hello").unwrap().as_ptr()),5);
            assert_eq!(buf[0],104);
            assert_eq!(buf[5],0);
            assert_eq!(buf[6], 89);
        }
    }

    // #[test]
    // fn test_flow_flow_snprintf_sufficeint_buffer(){
    //     unsafe {
    //         let mut buf:[c_char;7]= [23, 27, 25, 26,38,09,89 ];
    //         let raw_buf=buf.as_mut_ptr();
    //         assert_eq!(flow_snprintf(raw_buf,6,CString::new("hello").unwrap().as_ptr()),5);
    //         assert_eq!(buf[0],104);
    //         assert_eq!(buf[5],0);
    //         assert_eq!(buf[6], 89);
    //     }
    // }
}

