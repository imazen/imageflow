using System;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.IO;

namespace Imageflow.Fluent
{
    public interface IBytesSource: IDisposable
    {
        /// <summary>
        /// Return a reference to a byte array that (until the implementor is disposed) will (a) remain immutable, and (b) can be GC pinned.
        /// </summary>
        /// <returns></returns>
        Task<ArraySegment<byte>> GetBytesAsync(CancellationToken cancellationToken);
    }
    
    public struct BytesSource : IBytesSource
    {
        public BytesSource(byte[] bytes)
        {
            this.bytes = new ArraySegment<byte>(bytes, 0, bytes.Length);
        }
        public BytesSource(byte[] bytes, int offset, int length)
        {
            this.bytes = new ArraySegment<byte>(bytes, offset, length);
        }
        public BytesSource(ArraySegment<byte> bytes)
        {
            this.bytes = bytes;
        }
        
        private ArraySegment<byte> bytes;

        public void Dispose()
        {
        }
        public Task<ArraySegment<byte>> GetBytesAsync(CancellationToken cancellationToken) => Task.FromResult(bytes);
        
    }
    public class StreamSource : IBytesSource
    {
        internal static readonly RecyclableMemoryStreamManager Mgr = new RecyclableMemoryStreamManager();
        public StreamSource(Stream underlying, bool disposeUnderlying)
        {
            this._underlying = underlying;
            this._disposeUnderlying = disposeUnderlying;
        }
        private readonly Stream _underlying;
        private RecyclableMemoryStream _copy;
        private readonly bool _disposeUnderlying;
        public void Dispose()
        {
            if (_disposeUnderlying)
            {
                _underlying?.Dispose();
            }
            _copy?.Dispose();
        }

        public async Task<ArraySegment<byte>> GetBytesAsync(CancellationToken cancellationToken)
        {
            var length = _underlying.Length;
            if (length >= int.MaxValue) throw new OverflowException("Streams cannot exceed 2GB");
            switch (_underlying)
            {
                case RecyclableMemoryStream underlyingRecMemoryStream:
                    return new ArraySegment<byte>(underlyingRecMemoryStream.GetBuffer(), 0,
                        (int) length);
                case MemoryStream underlyingMemoryStream:
                    if (underlyingMemoryStream.TryGetBuffer(out var underlyingBuffer))
                    {
                        return underlyingBuffer;
                    }
                    break;
            }

            if (_copy == null)
            {
                _copy = new RecyclableMemoryStream(Mgr, "StreamSource: IBytesSource", (int) length);
                await _underlying.CopyToAsync(_copy);
            }
            
            return new ArraySegment<byte>(_copy.GetBuffer(), 0,
                (int) _copy.Length);
        }
    }
}