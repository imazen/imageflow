using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.IO;
using System.Reflection;
using System.Runtime.ConstrainedExecution;
using System.Runtime.InteropServices;
using System.Text;
using imageflow;
using Imageflow.Native;
using Newtonsoft.Json;

namespace Imageflow
{
    public class Context: CriticalFinalizerObject, IDisposable
    {
        private IntPtr _ptr;
        private List<GCHandle> pinned;
        internal IntPtr Pointer
        {
            get
            {
                if (_ptr == IntPtr.Zero) throw new ImageflowDisposedException("Context");
                return _ptr;
            }
        }

        public Context()
        {
            _ptr = NativeMethods.imageflow_context_create();
            if (_ptr == IntPtr.Zero) throw new OutOfMemoryException("Failed to create Imageflow Context");
        }

        internal void AddPinnedData(GCHandle handle)
        {
            if (pinned == null) pinned = new List<GCHandle>();
            pinned.Add(handle);
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
            return SendJsonBytes(method, Context.SerializeToJson(message));
        }

        public JsonResponse SendJsonBytes(string method, byte[] utf8Json)
        {
            
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
            if (pinned != null)
            {
                foreach (GCHandle active in pinned)
                    active.Free();
            }
            
            if (e != null) throw e;

        }

        ~Context()
        {
            Dispose (false);
        }
    }
}
