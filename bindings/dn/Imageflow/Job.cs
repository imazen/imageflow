using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography.X509Certificates;
using System.Text;
using imageflow;
using Imageflow.Native;
using Newtonsoft.Json;

namespace Imageflow
{
    public class Job: IDisposable
    {
        private IntPtr _ptr; 
        private readonly Context _parent;
        
        internal IntPtr Pointer
        {
            get
            {
                if (IsDisposed) throw new ImageflowDisposedException("Job");
                if (_parent.IsDisposed) throw new ImageflowDisposedException("Context");
                return _ptr;
            }
        }
        public Job(Context c)
        {
            _parent = c;
            
            c.AssertReady();
            this._ptr = NativeMethods.imageflow_job_create(c.Pointer);
            c.AssertReady();
            if (this._ptr == IntPtr.Zero) throw new ImageflowAssertionFailed("job pointer must be non-zero");
        }


        public void AddIo(JobIo io, int ioId, Direction direction)
        {
            _parent.AssertReady();
            NativeMethods.imageflow_job_add_io(_parent.Pointer, Pointer, io.Pointer, ioId, direction);
            _parent.AssertReady();
        }


        public JobIo GetIo(int ioId)
        {
            _parent.AssertReady();
            var ptr = NativeMethods.imageflow_job_get_io(_parent.Pointer, Pointer, ioId);
            _parent.AssertReady();
            return new JobIo(_parent, ptr);
        }



        public JsonResponse SendMessage<T>(string method, T message){
           return SendJsonBytes(method, Context.SerializeToJson(message));
        }

        public JsonResponse SendJsonBytes(string method, byte[] utf8Json)
        {
            
            var pinned = GCHandle.Alloc(utf8Json, GCHandleType.Pinned);
            var methodPinned = GCHandle.Alloc(Encoding.ASCII.GetBytes(method + char.MinValue), GCHandleType.Pinned);
            try
            {
                _parent.AssertReady();
                var ptr = NativeMethods.imageflow_job_send_json(_parent.Pointer, Pointer, methodPinned.AddrOfPinnedObject(), pinned.AddrOfPinnedObject(),
                    new UIntPtr((ulong) utf8Json.LongLength));
                _parent.AssertReady();
                return new JsonResponse(_parent, ptr);
            }
            finally
            {
                pinned.Free();
                methodPinned.Free();
            }
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

        public void AddOutputBuffer(int ioId)
        {
            AddIo(JobIo.OutputBuffer(_parent), ioId, Direction.Out);
        }
        
        public Stream GetOutputBuffer(int ioId)
        {
            return GetIo(ioId).OpenAsOutputBuffer();
        }

        public void AddInputBytesPinned(int ioId, byte[] buffer)
        {
            AddInputBytesPinned(ioId, buffer, 0, buffer.LongLength);
        }

        public void AddInputBytesPinned(int ioId, byte[] buffer, long offset, long count)
        {
            AddIo(JobIo.PinManagedBytes(_parent, buffer, offset, count), ioId, Direction.In);
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

            // Disposing the context also disposes the jobs 
            if (!_parent.IsDisposed)
            {
                if (!NativeMethods.imageflow_job_destroy(_parent.Pointer, _ptr))
                {
                    _ptr = IntPtr.Zero;
                    throw ImageflowException.FromContext(_parent);
                }
            }
            _ptr = IntPtr.Zero;

        }

        ~Job()
        {
            Dispose (false);
        }
        
    }
}
