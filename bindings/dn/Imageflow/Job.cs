using System;
using System.Runtime.InteropServices;
using System.Text;
using imageflow;
using Imageflow.Native;

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
            var jobPtr = NativeMethods.imageflow_job_create(c.Pointer);
            c.AssertReady();
            if (jobPtr == IntPtr.Zero) throw new ImageflowAssertionFailed("job pointer must be non-zero");
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


        public JsonResponse SendMessage(string method, byte[] utf8Json)
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
