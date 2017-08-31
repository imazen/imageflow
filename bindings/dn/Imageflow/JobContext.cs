using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.ConstrainedExecution;
using System.Runtime.InteropServices;
using System.Text;
using Imageflow.Native;
using Newtonsoft.Json;

namespace Imageflow
{
    public class JobContext: CriticalFinalizerObject, IDisposable, IAssertReady
    {
        private IntPtr _ptr;
        private List<GCHandle> _pinned;
        internal IntPtr Pointer
        {
            get
            {
                if (_ptr == IntPtr.Zero) throw new ImageflowDisposedException("JobContext");
                return _ptr;
            }
        }

        public JobContext()
        {
            _ptr = NativeMethods.imageflow_context_create(NativeMethods.ABI_MAJOR, NativeMethods.ABI_MINOR);
            if (_ptr != IntPtr.Zero) return;
            
            if (NativeMethods.imageflow_abi_compatible(NativeMethods.ABI_MAJOR, NativeMethods.ABI_MINOR))
            {
                throw new OutOfMemoryException("Failed to create Imageflow JobContext");
            }
            var major = NativeMethods.imageflow_abi_version_major();
            var minor = NativeMethods.imageflow_abi_version_minor();
            throw new Exception($".NET Imageflow bindings only support ABI {NativeMethods.ABI_MAJOR}.{NativeMethods.ABI_MINOR}. libimageflow ABI {major}.{minor} is loaded.");
        }

        internal void AddPinnedData(GCHandle handle)
        {
            if (_pinned == null) _pinned = new List<GCHandle>();
            _pinned.Add(handle);
        }

        public bool HasError => NativeMethods.imageflow_context_has_error(Pointer);
        
        public static byte[] SerializeToJson<T>(T obj){
            using (var stream = new MemoryStream())
            using (var writer = new StreamWriter(stream, new UTF8Encoding(false))){
                JsonSerializer.Create().Serialize(writer, obj);
                writer.Flush(); //Required or no bytes appear
                return stream.ToArray();
            }
        }
        
        public JsonResponse SendMessage<T>(string method, T message){
            AssertReady();
            return SendJsonBytes(method, JobContext.SerializeToJson(message));
        }

        public JsonResponse SendJsonBytes(string method, byte[] utf8Json)
        {
            AssertReady();
            var pinnedJson = GCHandle.Alloc(utf8Json, GCHandleType.Pinned);
            var methodPinned = GCHandle.Alloc(Encoding.ASCII.GetBytes(method + char.MinValue), GCHandleType.Pinned);
            try
            {
                AssertReady();
                var ptr = NativeMethods.imageflow_context_send_json(Pointer, methodPinned.AddrOfPinnedObject(), pinnedJson.AddrOfPinnedObject(),
                    new UIntPtr((ulong) utf8Json.LongLength));
                AssertReady();
                return new JsonResponse(this, ptr);
            }
            finally
            {
                pinnedJson.Free();
                methodPinned.Free();
            }
        }
        
        public void AssertReady()
        {
            if (HasError) throw ImageflowException.FromContext(this);
        }

        public JsonResponse ExecuteImageResizer4CommandString( int inputId, int outputId, string commands)
        {
            var message = new
            {
                framewise = new
                {
                    steps = new object[]
                    {
                        new
                        {
                            command_string = new
                            {
                                kind = "ir4",
                                value = "w=200&h=200&scale=both&format=jpg",
                                decode = inputId,
                                encode = outputId
                            }
                        }
                    }
                }
            };
                
            return SendMessage("v0.1/execute", message);
        }

      
      

        
        
//        internal void AddFile(int ioId, Direction direction,  IoMode mode, string path)
//        {
//            AssertReady();
//            var cpath = GCHandle.Alloc(Encoding.ASCII.GetBytes(path + char.MinValue), GCHandleType.Pinned);
//            try
//            {
//                if (!NativeMethods.imageflow_context_add_file(Pointer, ioId, direction, mode,
//                    cpath.AddrOfPinnedObject()))
//                {
//                    AssertReady();
//                    throw new ImageflowAssertionFailed("AssertReady should raise an exception if method fails");
//                }
//            } finally{
//                cpath.Free();
//            }
//        }
//
//        public void AddOutputFile(int ioId, string path) => AddFile(ioId,  Direction.Out, IoMode.WriteSeekable, path);
//        public void AddInputFile(int ioId, string path) => AddFile(ioId,  Direction.In, IoMode.ReadSeekable, path);

        public void AddInputBytes(int ioId, byte[] buffer)
        {
            AddInputBytes(ioId, buffer, 0, buffer.LongLength);
        }
        public void AddInputBytes(int ioId, byte[] buffer, long offset, long count)
        {
            AssertReady();
            if (offset < 0 || offset > buffer.LongLength - 1) throw new ArgumentOutOfRangeException("offset", offset, "Offset must be within array bounds");
            if (count < 0 || offset + count > buffer.LongLength) throw new ArgumentOutOfRangeException("count", count, "offset + count must be within array bounds. count cannot be negative");

            
            var fixedBytes = GCHandle.Alloc(buffer, GCHandleType.Pinned);
            try
            {
                var addr = new IntPtr(fixedBytes.AddrOfPinnedObject().ToInt64() + offset);

                if (!NativeMethods.imageflow_context_add_input_buffer(Pointer, ioId, addr, new UIntPtr((ulong) count),
                    Lifetime.OutlivesFunctionCall))
                {
                    AssertReady();
                    throw new ImageflowAssertionFailed("AssertReady should raise an exception if method fails");
                }
            } finally{
                fixedBytes.Free();
            }
        }


        public void AddInputBytesPinned(int ioId, byte[] buffer)
        {
            AddInputBytesPinned(ioId, buffer, 0, buffer.LongLength);
        }
        
        public void AddInputBytesPinned(int ioId, byte[] buffer, long offset, long count)
        {
            AssertReady();
            if (offset < 0 || offset > buffer.LongLength - 1)
                throw new ArgumentOutOfRangeException("offset", offset, "Offset must be within array bounds");
            if (count < 0 || offset + count > buffer.LongLength)
                throw new ArgumentOutOfRangeException("count", count,
                    "offset + count must be within array bounds. count cannot be negative");


            var fixedBytes = GCHandle.Alloc(buffer, GCHandleType.Pinned);
            AddPinnedData(fixedBytes);


            var addr = new IntPtr(fixedBytes.AddrOfPinnedObject().ToInt64() + offset);
            if (!NativeMethods.imageflow_context_add_input_buffer(Pointer, ioId, addr, new UIntPtr((ulong) count),
                Lifetime.OutlivesContext))
            {
                AssertReady();
                throw new ImageflowAssertionFailed("AssertReady should raise an exception if method fails");
            }

        }


        public void AddOutputBuffer(int ioId)
        {
            AssertReady();
            if (!NativeMethods.imageflow_context_add_output_buffer(Pointer, ioId))
            {
                AssertReady();
                throw new ImageflowAssertionFailed("AssertReady should raise an exception if method fails");
            }
        }

        /// <summary>
        /// Will raise an unrecoverable exception if this is not an output buffer
        /// </summary>
        /// <returns></returns>
        public Stream GetOutputBuffer(int ioId)
        {
            AssertReady();
            IntPtr buffer;
            UIntPtr bufferSize;
            AssertReady();
            if (!NativeMethods.imageflow_context_get_output_buffer_by_id(Pointer, ioId, out buffer,
                out bufferSize))
            {
                AssertReady();
                throw new ImageflowAssertionFailed("AssertReady should raise an exception if method fails");
            }
            return new ImageflowUnmanagedReadStream(this, buffer, bufferSize);
            
        }
        public bool IsDisposed => _ptr == IntPtr.Zero; 
        public void Dispose()
        {
            Dispose(true);
            GC.SuppressFinalize(this);
        }

        protected virtual void Dispose(bool disposing)
        {
            if (_ptr == IntPtr.Zero) return;

            if (disposing)
            {
                // Free managed objects
            }

            Exception e = null;
            if (!NativeMethods.imageflow_context_begin_terminate(_ptr))
            {
                e = ImageflowException.FromContext(this);
            }
            NativeMethods.imageflow_context_destroy(_ptr);
            _ptr = IntPtr.Zero;
            
            //Unpin all managed data held for context lifetime
            if (_pinned != null)
            {
                foreach (var active in _pinned)
                    active.Free();
            }
            
            if (e != null) throw e;

        }

        ~JobContext()
        {
            Dispose (false);
        }
        
    }
}
