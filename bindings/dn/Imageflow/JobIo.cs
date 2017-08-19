using System;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using imageflow;
using Imageflow.Native;

namespace Imageflow
{
    public class JobIo : IAssertReady
    {
        private IntPtr _ptr; 
        private readonly Context _parent;
        
        internal IntPtr Pointer
        {
            get
            {
                if (_parent.IsDisposed) throw new ImageflowDisposedException("Context");
                return _ptr;
            }
        }

        internal static JobIo ForFile(Context c, string path, IoMode mode)
        {
            var cpath = GCHandle.Alloc(Encoding.ASCII.GetBytes(path + char.MinValue), GCHandleType.Pinned);
            try
            {
                return new JobIo(c,
                    NativeMethods.imageflow_io_create_for_file(c.Pointer, mode,
                        cpath.AddrOfPinnedObject(),
                        CleanupWith.Context));
            } finally{
                cpath.Free();
            }
        }

        public static JobIo WriteToFile(Context c, string path) => ForFile(c, path, IoMode.WriteSeekable);
        public static JobIo ReadFromFile(Context c, string path) => ForFile(c, path, IoMode.ReadSeekable);

        public static JobIo ReadBytes(Context c, byte[] bytes)
        {
            var fixedBytes = GCHandle.Alloc(bytes, GCHandleType.Pinned);
            try
            {
                return new JobIo(c,
                    NativeMethods.imageflow_io_create_from_buffer(c.Pointer, fixedBytes.AddrOfPinnedObject(), new UIntPtr((ulong)bytes.LongLength), Lifetime.OutlivesFunctionCall, 
                        CleanupWith.Context));
            } finally{
                fixedBytes.Free();
            }
        }
        
        public static JobIo OutputBuffer(Context c)
        {
            return new JobIo(c,
                    NativeMethods.imageflow_io_create_for_output_buffer(c.Pointer));
        }

        /// <summary>
        /// Will raise an unrecoverable exception if this is not an output buffer
        /// </summary>
        /// <returns></returns>
        public Stream OpenAsOutputBuffer()
        {

            IntPtr buffer;
            UIntPtr bufferSize;
            _parent.AssertReady();
            if (!NativeMethods.imageflow_io_get_output_buffer(_parent.Pointer, Pointer, out buffer,
                out bufferSize))
            {
                _parent.AssertReady();
                throw new ImageflowAssertionFailed("AssertReady should raise an exception if method fails");
            }
            return new ImageflowUnmanagedReadStream(this, buffer, bufferSize);
            
        }
        
        internal JobIo(Context c, IntPtr _ptr)
        {
            _parent = c;
            this._ptr = _ptr; 
            c.AssertReady();
            if (_ptr == IntPtr.Zero) throw new ImageflowAssertionFailed("JobIo pointer must be non-zero");
        }
        
        public void AssertReady()
        {
            _parent.AssertReady();
        }
    }
}
