using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using imageflow;
using Imageflow.Native;
using Newtonsoft.Json;

namespace Imageflow
{
    public class JsonResponse : IDisposable, IAssertReady
    {
        private IntPtr _ptr;
        private readonly Context _parent;

        internal IntPtr Pointer
        {
            get
            {
                if (IsDisposed) throw new ImageflowDisposedException("JsonResponse");
                if (_parent.IsDisposed) throw new ImageflowDisposedException("Context");
                return _ptr;
            }
        }

        public JsonResponse(Context c, IntPtr ptr)
        {
            _parent = c;
            this._ptr = ptr;
            c.AssertReady();
            if (ptr == IntPtr.Zero) throw new ImageflowAssertionFailed("JsonResponse pointer must be non-zero");
        }

        private void Read(out int statusCode, out IntPtr utf8Buffer, out UIntPtr bufferSize)
        {
            _parent.AssertReady();
            NativeMethods.imageflow_json_response_read(_parent.Pointer, Pointer, out statusCode, out utf8Buffer,
                out bufferSize);
            _parent.AssertReady();
        }

        public int GetStatusCode()
        {
            int statusCode;
            IntPtr utf8Buffer;
            UIntPtr bufferSize;
            Read(out statusCode, out utf8Buffer, out bufferSize);
            return statusCode;
        }

        public Stream GetStream()
        {
            int statusCode;
            IntPtr utf8Buffer;
            UIntPtr bufferSize;
            Read(out statusCode, out utf8Buffer, out bufferSize);
            return new ImageflowUnmanagedReadStream(this, utf8Buffer, bufferSize);
        }

        public T Deserialize<T>() where T : class
        {
            using (var reader = new StreamReader(GetStream(), Encoding.UTF8))
                return JsonSerializer.Create().Deserialize((JsonReader) new JsonTextReader(reader), typeof(T)) as T;
        }

        public dynamic DeserializeDynamic()
        {
            using (var reader = new StreamReader(GetStream(), Encoding.UTF8))
                return JsonSerializer.Create().Deserialize(new JsonTextReader(reader));
        }

        public string GetString() => new StreamReader(GetStream(), Encoding.UTF8).ReadToEnd();
    

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
                if (!NativeMethods.imageflow_json_response_destroy(_parent.Pointer, _ptr))
                {
                    _ptr = IntPtr.Zero;
                    throw ImageflowException.FromContext(_parent);
                }
            }
            _ptr = IntPtr.Zero;

        }

        ~JsonResponse()
        {
            Dispose(false);
        }

        public void AssertReady()
        {
            _parent.AssertReady();
            if (this.Pointer == IntPtr.Zero) throw new ImageflowAssertionFailed("Pointer must never return zero");
        }

    }
}
