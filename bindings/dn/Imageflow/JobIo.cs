using System;
using System.Diagnostics;
using System.IO;
using System.Net.Sockets;
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

        public static JobIo CopyBytes(Context c, byte[] buffer)
        {
            return CopyBytes(c, buffer, 0, buffer.LongLength);
        }
        public static JobIo CopyBytes(Context c, byte[] buffer, long offset, long count)
        {
            if (offset < 0 || offset > buffer.LongLength - 1) throw new ArgumentOutOfRangeException("offset", offset, "Offset must be within array bounds");
            if (count < 0 || offset + count > buffer.LongLength) throw new ArgumentOutOfRangeException("count", count, "offset + count must be within array bounds. count cannot be negative");

            
            var fixedBytes = GCHandle.Alloc(buffer, GCHandleType.Pinned);
            try
            {
                var addr = new IntPtr(fixedBytes.AddrOfPinnedObject().ToInt64() + offset);
                return new JobIo(c,
                    NativeMethods.imageflow_io_create_from_buffer(c.Pointer, addr , new UIntPtr((ulong)count), Lifetime.OutlivesFunctionCall, 
                        CleanupWith.Context));
            } finally{
                fixedBytes.Free();
            }
        }


        public static JobIo PinManagedBytes(Context c, byte[] buffer)
        {
            return PinManagedBytes(c, buffer, 0, buffer.LongLength);
        }
        
        public static JobIo PinManagedBytes(Context c, byte[] buffer, long offset, long count)
        {
            if (offset < 0 || offset > buffer.LongLength - 1)
                throw new ArgumentOutOfRangeException("offset", offset, "Offset must be within array bounds");
            if (count < 0 || offset + count > buffer.LongLength)
                throw new ArgumentOutOfRangeException("count", count,
                    "offset + count must be within array bounds. count cannot be negative");


            var fixedBytes = GCHandle.Alloc(buffer, GCHandleType.Pinned);
            c.AddPinnedData(fixedBytes);


            var addr = new IntPtr(fixedBytes.AddrOfPinnedObject().ToInt64() + offset);
            return new JobIo(c,
                NativeMethods.imageflow_io_create_from_buffer(c.Pointer, addr, new UIntPtr((ulong) count),
                    Lifetime.OutlivesContext,
                    CleanupWith.Context));

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
        
        internal JobIo(Context c, IntPtr ptr)
        {
            _parent = c;
            this._ptr = ptr; 
            c.AssertReady();
            if (ptr == IntPtr.Zero) throw new ImageflowAssertionFailed("JobIo pointer must be non-zero");
        }
        
        public void AssertReady()
        {
            _parent.AssertReady();
        }
    }
}
