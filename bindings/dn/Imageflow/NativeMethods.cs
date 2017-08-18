using System;
using System.Runtime.InteropServices;

namespace Imageflow.Native
{

    public enum IoMode
    {

        /// None -> 0
        None = 0,

        /// ReadSequential -> 1
        ReadSequential = 1,

        /// WriteSequential -> 2
        WriteSequential = 2,

        /// ReadSeekable -> 5
        ReadSeekable = 5,

        /// WriteSeekable -> 6
        WriteSeekable = 6,

        /// ReadWriteSeekable -> 15
        ReadWriteSeekable = 15,
    }

    public enum Direction
    {

        /// Out -> 8
        Out = 8,

        /// In -> 4
        In = 4,
    }

    public enum CleanupWith
    {

        /// Context -> 0
        Context = 0,

        /// FirstJob -> 1
        FirstJob = 1,
    }

    public enum Lifetime
    {

        /// OutlivesFunctionCall -> 0
        OutlivesFunctionCall = 0,

        /// OutlivesContext -> 1
        OutlivesContext = 1,
    }

    public class NativeMethods
    {
        [DllImport("imageflow")] 
        public static extern IntPtr imageflow_context_create();

        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_context_begin_terminate(IntPtr context);

        [DllImport("imageflow")] 
        public static extern void imageflow_context_destroy(IntPtr context);

        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_context_has_error(IntPtr context);

        [DllImport("imageflow")] 
        public static extern void imageflow_context_clear_error(IntPtr context);

        /// Return Type: int32_t->int
        [DllImport("imageflow")] 
        public static extern int imageflow_context_error_code(IntPtr context);

        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_context_print_and_exit_if_error(IntPtr context);


        
        ///error_code: int32_t->int
        ///message: char*
        ///filename: char*
        ///line: int32_t->int
        ///function_name: char*
        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_context_raise_error(IntPtr context, int error_code,
            IntPtr message, IntPtr filename, int line, IntPtr function_name);


        
        ///filename: char*
        ///line: int32_t->int
        ///function_name: char*
        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_context_add_to_callstack(IntPtr context, IntPtr filename,
            int line, IntPtr function_name);


        
        ///response_in: void*
        ///status_code_out: int64_t*
        ///buffer_utf8_no_nulls_out: uint8_t**
        ///buffer_size_out: size_t*
        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_json_response_read(IntPtr context, IntPtr response_in,
            ref int status_code_out, ref IntPtr buffer_utf8_no_nulls_out, ref UIntPtr buffer_size_out);


     
        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_json_response_destroy(IntPtr context, IntPtr response);


        /// Return Type: void*
        ///context: void*
        ///mode: IoMode
        ///filename: char*
        ///cleanup: CleanupWith
        [DllImport("imageflow")] 
        public static extern IntPtr imageflow_io_create_for_file(IntPtr context, IoMode mode,
            IntPtr filename, CleanupWith cleanup);




        /// Return Type: void*
        ///context: void*
        [DllImport("imageflow")] 
        public static extern IntPtr imageflow_io_create_for_output_buffer(IntPtr context);


        
        ///io: void*
        ///result_buffer: uint8_t**
        ///result_buffer_length: size_t*
        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_io_get_output_buffer(IntPtr context, IntPtr io,
            ref IntPtr result_buffer, ref UIntPtr result_buffer_length);


        
        ///job: void*
        ///io_id: int32_t->int
        ///result_buffer: uint8_t**
        ///result_buffer_length: size_t*
        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_job_get_output_buffer_by_id(IntPtr context, IntPtr job,
            int io_id, ref IntPtr result_buffer, ref UIntPtr result_buffer_length);


        /// Return Type: void*
        ///context: void*
        [DllImport("imageflow")] 
        public static extern IntPtr imageflow_job_create(IntPtr context);


        /// Return Type: void*
        ///context: void*
        ///job: void*
        ///io_id: int32_t->int
        [DllImport("imageflow")] 
        public static extern IntPtr imageflow_job_get_io(IntPtr context, IntPtr job, int io_id);


        
        ///job: void*
        ///io: void*
        ///io_id: int32_t->int
        ///direction: Direction
        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_job_add_io(IntPtr context, IntPtr job, IntPtr io,
            int io_id, Direction direction);


        
        ///job: void*
        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_job_destroy(IntPtr context, IntPtr job);



        
        ///pointer: void*
        ///filename: char*
        ///line: int32_t->int
        [DllImport("imageflow")] 
        [return: MarshalAs(UnmanagedType.I1)]
        public static extern bool imageflow_context_memory_free(IntPtr context, IntPtr pointer,
            IntPtr filename, int line);


        [DllImport("imageflow")] 
        public static extern int imageflow_context_error_and_stacktrace(IntPtr context, IntPtr buffer,
            UIntPtr buffer_length,
            [MarshalAs(UnmanagedType.I1)]
            bool full_file_path);


        [DllImport("imageflow")] 
        public static extern IntPtr imageflow_context_send_json(IntPtr context, IntPtr method,
            IntPtr json_buffer, UIntPtr json_buffer_size);


        [DllImport("imageflow")] 
        public static extern IntPtr imageflow_job_send_json(IntPtr context, IntPtr job,
            IntPtr method, IntPtr json_buffer, UIntPtr json_buffer_size);


        [DllImport("imageflow")] 
        public static extern IntPtr imageflow_io_create_from_buffer(IntPtr context, IntPtr buffer,
            UIntPtr buffer_byte_count, Lifetime lifetime, CleanupWith cleanup);


        [DllImport("imageflow")] 
        public static extern IntPtr imageflow_context_memory_allocate(IntPtr context, IntPtr bytes,
            IntPtr filename, int line);

    }
}

