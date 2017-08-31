using System;
using System.Runtime.InteropServices;

namespace Imageflow.Native
{

//    public enum IoMode
//    {
//
//        /// None -> 0
//        None = 0,
//
//        /// ReadSequential -> 1
//        ReadSequential = 1,
//
//        /// WriteSequential -> 2
//        WriteSequential = 2,
//
//        /// ReadSeekable -> 5
//        ReadSeekable = 5,
//
//        /// WriteSeekable -> 6
//        WriteSeekable = 6,
//
//        /// ReadWriteSeekable -> 15
//        ReadWriteSeekable = 15,
//    }
//
//    public enum Direction
//    {
//
//        /// Out -> 8
//        Out = 8,
//
//        /// In -> 4
//        In = 4,
//    }

    public enum Lifetime
    {

        /// OutlivesFunctionCall -> 0
        OutlivesFunctionCall = 0,

        /// OutlivesContext -> 1
        OutlivesContext = 1,
    }

    public class NativeMethods
    {
        
        public const int ABI_MAJOR = 3;
        public const int ABI_MINOR = 0;
        
        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_abi_compatible(uint imageflow_abi_ver_major, uint imageflow_abi_ver_minor);

        [DllImport("imageflow")] 
        public static extern uint imageflow_abi_version_major();
        
        [DllImport("imageflow")] 
        public static extern uint imageflow_abi_version_minor();
        
        [DllImport("imageflow")] 
        public static extern IntPtr imageflow_context_create(uint imageflow_abi_ver_major, uint imageflow_abi_ver_minor);

        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_context_begin_terminate(IntPtr context);

        [DllImport("imageflow")] 
        public static extern void imageflow_context_destroy(IntPtr context);

        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_context_has_error(IntPtr context);

        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_context_error_recoverable(IntPtr context);

        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_context_error_try_clear(IntPtr context);

        /// Return Type: int32_t->int
        [DllImport("imageflow")] 
        public static extern int imageflow_context_error_code(IntPtr context);

        [DllImport("imageflow")] 
        public static extern int imageflow_context_error_as_exit_code(IntPtr context);

        [DllImport("imageflow")] 
        public static extern int imageflow_context_error_as_http_code(IntPtr context);

        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_context_print_and_exit_if_error(IntPtr context);


        
        ///response_in: void*
        ///status_code_out: int64_t*
        ///buffer_utf8_no_nulls_out: uint8_t**
        ///buffer_size_out: size_t*
        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_json_response_read(IntPtr context, IntPtr response_in,
            out int status_code_out, out IntPtr buffer_utf8_no_nulls_out, out UIntPtr buffer_size_out);


     
        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_json_response_destroy(IntPtr context, IntPtr response);



        
        ///pointer: void*
        ///filename: char*
        ///line: int32_t->int
        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_context_memory_free(IntPtr context, IntPtr pointer,
            IntPtr filename, int line);

        
        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_context_error_write_to_buffer(IntPtr context, IntPtr buffer,
            UIntPtr buffer_length,
            out UIntPtr bytes_written);


        [DllImport("imageflow")] 
        public static extern IntPtr imageflow_context_send_json(IntPtr context, IntPtr method,
            IntPtr json_buffer, UIntPtr json_buffer_size);


        

        [DllImport("imageflow")] 
        public static extern IntPtr imageflow_context_memory_allocate(IntPtr context, IntPtr bytes,
            IntPtr filename, int line);
        
        
        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool  imageflow_context_add_input_buffer(IntPtr context, int io_id, IntPtr buffer,
            UIntPtr buffer_byte_count, Lifetime lifetime);

        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_context_add_output_buffer(IntPtr context, int io_id);
        
//        [DllImport("imageflow")] 
//        [return: MarshalAs(UnmanagedType.I1)]
//        public static extern bool  imageflow_context_add_file(IntPtr context, int io_id,Direction direction,  IoMode mode,
//            IntPtr filename);


        
        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_context_get_output_buffer_by_id(IntPtr context,
            int io_id, out IntPtr result_buffer, out UIntPtr result_buffer_length);


    }
}

