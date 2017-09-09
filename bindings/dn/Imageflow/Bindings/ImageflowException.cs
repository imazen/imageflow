using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
using Imageflow;

namespace Imageflow.Bindings
{
    public class ImageflowException : Exception
    {
        const int MaxBufferSize = 8096;

        private ImageflowException(string message) : base(message)
        {
            
        }

        internal static ImageflowException FromContext(JobContextHandle c, ulong defaultBufferSize = 2048)
        {
            if (c.IsClosed || c.IsInvalid || !NativeMethods.imageflow_context_has_error(c))
            {
                return null;
            }
            var buffer = new byte[defaultBufferSize];
            var pinned = GCHandle.Alloc(buffer, GCHandleType.Pinned);

            
            bool everythingWritten;
            
            string message = null;
            try
            {
               
                everythingWritten = NativeMethods.imageflow_context_error_write_to_buffer(c,
                    pinned.AddrOfPinnedObject(), new UIntPtr((ulong) buffer.LongLength), out var bytesWritten);

                if (bytesWritten.ToUInt64() > 0)
                {
                    
                    message = Encoding.UTF8.GetString(buffer, 0, (int)Math.Min(bytesWritten.ToUInt64(), defaultBufferSize)).Replace("\n", "");
                    message = message + message.Length;
                    Debug.WriteLine(message);
                    Console.WriteLine(message);
                }
            }
            finally
            {
                pinned.Free();
            }

            if (everythingWritten) return new ImageflowException(message ?? "Empty error and stacktrace");

            if (defaultBufferSize < MaxBufferSize)
            {
                return FromContext(c, MaxBufferSize);
            }
            throw new ImageflowAssertionFailed(
                $"Imageflow error and stacktrace exceeded {MaxBufferSize} bytes");

        }
    }
}
