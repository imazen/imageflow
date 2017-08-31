using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
using Imageflow;
using Imageflow.Native;

namespace Imageflow
{
    public class ImageflowException : Exception
    {
        const int MaxBufferSize = 8096;

        internal ImageflowException(string message) : base(message)
        {
            
        }

        public static Exception FromContext(JobContext c, bool fullPaths = true, ulong defaultBufferSize = 2048)
        {
            if (!NativeMethods.imageflow_context_has_error(c.Pointer))
            {
                return null;
            }
            var buffer = new byte[defaultBufferSize];
            var pinned = GCHandle.Alloc(buffer, GCHandleType.Pinned);

            var bytesWritten = UIntPtr.Zero;
            var everythingWritten = false;
            
            string message = null;
            try
            {
                everythingWritten = NativeMethods.imageflow_context_error_write_to_buffer(c.Pointer,
                    pinned.AddrOfPinnedObject(), new UIntPtr((ulong) buffer.LongLength), out bytesWritten);

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
                return FromContext(c, fullPaths, MaxBufferSize);
            }
            throw new ImageflowAssertionFailed(
                $"Imageflow error and stacktrace exceeded {MaxBufferSize} bytes");

        }
    }
}
