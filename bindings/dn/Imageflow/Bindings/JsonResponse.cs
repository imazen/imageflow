using System;
using System.IO;
using System.Text;
using Newtonsoft.Json;

namespace Imageflow.Bindings
{
    /// <summary>
    /// Readable even if the JobContext is in an error state.
    /// </summary>
    public sealed class JsonResponse : IDisposable, IAssertReady
    {
        private readonly JsonResponseHandle _handle;

        private JsonResponseHandle Handle
        {
            get
            {
                AssertReady();
                return _handle;
            }
        }

        internal JsonResponse(JsonResponseHandle ptr)
        {
            
            ptr.ParentContext.AssertReady();
            ptr.AssertReady();
            _handle = ptr;
        }

        public void AssertReady()
        {
            _handle.ParentContext.AssertReady();
            if (!_handle.IsValid) throw new ObjectDisposedException("Imageflow JsonResponse");
        }

        private void Read(out int statusCode, out IntPtr utf8Buffer, out UIntPtr bufferSize)
        {
            AssertReady();
            NativeMethods.imageflow_json_response_read(_handle.ParentContext, Handle, out statusCode, out utf8Buffer,
                out bufferSize);
            AssertReady();
        }

        public int GetStatusCode()
        {
            Read(out var statusCode, out var utf8Buffer, out var bufferSize);
            return statusCode;
        }

        public Stream GetStream()
        {
            Read(out var statusCode, out var utf8Buffer, out var bufferSize);
            return new ImageflowUnmanagedReadStream(this, utf8Buffer, bufferSize);
        }

        public T Deserialize<T>() where T : class
        {
            using (var reader = new StreamReader(GetStream(), Encoding.UTF8))
                return JsonSerializer.Create().Deserialize(new JsonTextReader(reader), typeof(T)) as T;
        }

        public dynamic DeserializeDynamic()
        {
            using (var reader = new StreamReader(GetStream(), Encoding.UTF8))
                return JsonSerializer.Create().Deserialize(new JsonTextReader(reader));
        }

        public string GetString() => new StreamReader(GetStream(), Encoding.UTF8).ReadToEnd();


        public bool IsDisposed => !_handle.IsValid;

        public void Dispose()
        {
            var e = _handle.DisposeAllowingException();
            if (e != null) throw e;
        }

    }
}
