using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.ConstrainedExecution;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;
using Newtonsoft.Json;

namespace Imageflow.Bindings
{

    public sealed class JobContext: CriticalFinalizerObject, IDisposable, IAssertReady
    {
        private readonly JobContextHandle _handle;
        private List<GCHandle> _pinned;
        private List<IDisposable> _toDispose;
        internal JobContextHandle Handle
        {
            get
            {
                if (!_handle.IsValid)  throw new ObjectDisposedException("Imageflow JobContext");
                return _handle;
            }
        }
        private enum IoKind { InputBuffer, OutputBuffer}

        internal bool IsInput(int ioId) => ioSet.ContainsKey(ioId) && ioSet[ioId] == IoKind.InputBuffer;
        internal bool IsOutput(int ioId) => ioSet.ContainsKey(ioId) && ioSet[ioId] == IoKind.OutputBuffer;
        internal int LargestIoId => ioSet.Keys.DefaultIfEmpty().Max();
        
        private Dictionary<int, IoKind> ioSet = new Dictionary<int, IoKind>();

        public JobContext()
        {
            _handle = new JobContextHandle();
        }

        private void AddPinnedData(GCHandle handle)
        {
            if (_pinned == null) _pinned = new List<GCHandle>();
            _pinned.Add(handle);
        }

        public bool HasError => NativeMethods.imageflow_context_has_error(Handle);
        
        private static byte[] SerializeToJson<T>(T obj){
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
        
        public JsonResponse Execute<T>(T message){
            AssertReady();
            return SendJsonBytes("v0.1/execute", JobContext.SerializeToJson(message));
        }

        public JsonResponse SendJsonBytes(string method, byte[] utf8Json)
        {
            AssertReady();
            var pinnedJson = GCHandle.Alloc(utf8Json, GCHandleType.Pinned);
            var methodPinned = GCHandle.Alloc(Encoding.ASCII.GetBytes(method + char.MinValue), GCHandleType.Pinned);
            try
            {
                AssertReady();
                var ptr = NativeMethods.imageflow_context_send_json(Handle, methodPinned.AddrOfPinnedObject(), pinnedJson.AddrOfPinnedObject(),
                    new UIntPtr((ulong) utf8Json.LongLength));
                AssertReady();
                return new JsonResponse(new JsonResponseHandle(_handle, ptr));
            }
            finally
            {
                pinnedJson.Free();
                methodPinned.Free();
            }
        }
        
        public void AssertReady()
        {
            if (!_handle.IsValid)  throw new ObjectDisposedException("Imageflow JobContext");
            if (HasError) throw ImageflowException.FromContext(Handle);
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
                                value = commands,
                                decode = inputId,
                                encode = outputId
                            }
                        }
                    }
                }
            };
                
            return Execute( message);
        }



        internal void AddToDisposeQueue(IDisposable d)
        {
            if (this._toDispose == null) this._toDispose = new List<IDisposable>(1);
            _toDispose.Add(d);
        }
        
        
//        internal void AddFile(int ioId, Direction direction,  IoMode mode, string path)
//        {
//            AssertReady();
//            var cpath = GCHandle.Alloc(Encoding.ASCII.GetBytes(path + char.MinValue), GCHandleType.Pinned);
//            try
//            {
//                if (!NativeMethods.imageflow_context_add_file(Handle, ioId, direction, mode,
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
        public void AddInputBytes(int ioId, ArraySegment<byte> buffer)
        {
            AddInputBytes(ioId, buffer.Array, buffer.Offset, buffer.Count);
        }
        public void AddInputBytes(int ioId, byte[] buffer, long offset, long count)
        {
            AssertReady();
            if (offset < 0 || offset > buffer.LongLength - 1) throw new ArgumentOutOfRangeException("offset", offset, "Offset must be within array bounds");
            if (count < 0 || offset + count > buffer.LongLength) throw new ArgumentOutOfRangeException("count", count, "offset + count must be within array bounds. count cannot be negative");
            if (ContainsIoId(ioId)) throw new ArgumentException($"ioId {ioId} already in use", "ioId");
            
            var fixedBytes = GCHandle.Alloc(buffer, GCHandleType.Pinned);
            try
            {
                var addr = new IntPtr(fixedBytes.AddrOfPinnedObject().ToInt64() + offset);

                if (!NativeMethods.imageflow_context_add_input_buffer(Handle, ioId, addr, new UIntPtr((ulong) count),
                    NativeMethods.Lifetime.OutlivesFunctionCall))
                {
                    AssertReady();
                    throw new ImageflowAssertionFailed("AssertReady should raise an exception if method fails");
                }
                ioSet.Add(ioId, IoKind.InputBuffer);
            } finally{
                fixedBytes.Free();
            }
        }


        public void AddInputBytesPinned(int ioId, byte[] buffer)
        {
            AddInputBytesPinned(ioId, buffer, 0, buffer.LongLength);
        }
        public void AddInputBytesPinned(int ioId, ArraySegment<byte> buffer)
        {
            AddInputBytesPinned(ioId, buffer.Array, buffer.Offset, buffer.Count);
        }
        public void AddInputBytesPinned(int ioId, byte[] buffer, long offset, long count)
        {
            AssertReady();
            if (offset < 0 || offset > buffer.LongLength - 1)
                throw new ArgumentOutOfRangeException("offset", offset, "Offset must be within array bounds");
            if (count < 0 || offset + count > buffer.LongLength)
                throw new ArgumentOutOfRangeException("count", count,
                    "offset + count must be within array bounds. count cannot be negative");
            if (ContainsIoId(ioId)) throw new ArgumentException($"ioId {ioId} already in use", "ioId");

            var fixedBytes = GCHandle.Alloc(buffer, GCHandleType.Pinned);
            AddPinnedData(fixedBytes);

            var addr = new IntPtr(fixedBytes.AddrOfPinnedObject().ToInt64() + offset);
            if (!NativeMethods.imageflow_context_add_input_buffer(Handle, ioId, addr, new UIntPtr((ulong) count),
                NativeMethods.Lifetime.OutlivesContext))
            {
                AssertReady();
                throw new ImageflowAssertionFailed("AssertReady should raise an exception if method fails");
            }
            ioSet.Add(ioId, IoKind.InputBuffer);
        }


        public void AddOutputBuffer(int ioId)
        {
            AssertReady();
            if (ContainsIoId(ioId)) throw new ArgumentException($"ioId {ioId} already in use", "ioId");
            if (!NativeMethods.imageflow_context_add_output_buffer(Handle, ioId))
            {
                AssertReady();
                throw new ImageflowAssertionFailed("AssertReady should raise an exception if method fails");
            }
            ioSet.Add(ioId, IoKind.OutputBuffer);
        }

        public bool ContainsIoId(int ioId) => ioSet.ContainsKey(ioId);
        
        /// <summary>
        /// Will raise an unrecoverable exception if this is not an output buffer
        /// </summary>
        /// <returns></returns>
        public Stream GetOutputBuffer(int ioId)
        {
            if (!ioSet.ContainsKey(ioId) || ioSet[ioId] != IoKind.OutputBuffer)
            {
                throw new ArgumentException($"ioId {ioId} does not correspond to an output buffer", "ioId");
            }
            AssertReady();
            if (!NativeMethods.imageflow_context_get_output_buffer_by_id(Handle, ioId, out var buffer,
                out var bufferSize))
            {
                AssertReady();
                throw new ImageflowAssertionFailed("AssertReady should raise an exception if method fails");
            }
            return new ImageflowUnmanagedReadStream(this, buffer, bufferSize);
            
        }

        public bool IsDisposed => !_handle.IsValid;
        public void Dispose()
        {
            if (IsDisposed) throw new ObjectDisposedException("Imageflow JobContext");
            
            // Do not allocate or throw exceptions unless (disposing)
            Exception e = null;
            try
            {
                e = _handle.DisposeAllowingException();
            }
            finally
            {
                UnpinAll();
                
                //Dispose all managed data held for context lifetime
                if (_toDispose != null)
                {
                    foreach (var active in _toDispose)
                        active.Dispose();
                    _toDispose = null;
                }
                GC.SuppressFinalize(this);
                if (e != null) throw e;
            } 
        }

        private void UnpinAll()
        {
            //Unpin GCHandles
            if (_pinned != null)
            {
                foreach (var active in _pinned)
                {
                    if (active.IsAllocated) active.Free();
                }
                _pinned = null;
            }
        }

        ~JobContext()
        {
            //Don't dispose managed objects; they have their own finalizers
            UnpinAll();
        }
        
    }
}
