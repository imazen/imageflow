using System;
using System.IO;
using System.Threading;
using System.Threading.Tasks;

namespace Imageflow.Bindings
{
    
    /// <summary>
    /// An UnmanagedMemoryStream that checks that the underlying Imageflow context isn't in a disposed or errored state
    /// </summary>
    /// <inheritdoc cref="UnmanagedMemoryStream"/>
    public sealed class ImageflowUnmanagedReadStream : UnmanagedMemoryStream
    {
        private readonly IAssertReady _underlying;
        
        internal unsafe ImageflowUnmanagedReadStream(IAssertReady underlying, IntPtr buffer, UIntPtr length) : base( (byte*)buffer.ToPointer(), (long)length.ToUInt64(), (long)length.ToUInt64(), FileAccess.Read)
        {
            _underlying = underlying;
        }

        private void CheckSafe()
        {    
            _underlying.AssertReady();
        }
        public override int Read(byte[] buffer, int offset, int count)
        {
            CheckSafe();
            return base.Read(buffer, offset, count);
        }

        public override Task<int> ReadAsync(byte[] buffer, int offset, int count, CancellationToken cancellationToken)
        {
            CheckSafe();
            return base.ReadAsync(buffer, offset, count, cancellationToken);
        }

        public override int ReadByte()
        {
            CheckSafe();
            return base.ReadByte();
        }

        public override IAsyncResult BeginRead(byte[] buffer, int offset, int count, AsyncCallback callback, object state)
        {
            CheckSafe();
            return base.BeginRead(buffer, offset, count, callback, state);
        }

        public override int EndRead(IAsyncResult asyncResult)
        {
            CheckSafe();
            return base.EndRead(asyncResult);
        }

        public override Task CopyToAsync(Stream destination, int bufferSize, CancellationToken cancellationToken)
        {
            CheckSafe();
            return base.CopyToAsync(destination, bufferSize, cancellationToken);
        }
    }
}
