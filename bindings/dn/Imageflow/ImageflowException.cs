using System;
using System.Runtime.InteropServices;
using System.Text;
using Imageflow;
using Imageflow.Native;

namespace imageflow
{
    public class ImageflowException : Exception
    {
        const int MaxBufferSize = 8096;

        internal ImageflowException(string message) : base(message)
        {
            
        }

        public static Exception FromContext(Context c, bool fullPaths = true, int defaultBufferSize = 2048)
        {
            if (!NativeMethods.imageflow_context_has_error(c.Pointer))
            {
                return null;
            }
            var buffer = new byte[defaultBufferSize];
            var pinned = GCHandle.Alloc(buffer, GCHandleType.Pinned);

            int bytesWritten;
            string message = null;
            try
            {
                bytesWritten = NativeMethods.imageflow_context_error_and_stacktrace(c.Pointer,
                    pinned.AddrOfPinnedObject(), new UIntPtr((ulong) buffer.LongLength), fullPaths);

                if (bytesWritten > 0)
                {
                    message = Encoding.UTF8.GetString(buffer, 0, Math.Min(bytesWritten, defaultBufferSize));
                }
            }
            finally
            {
                pinned.Free();
            }

            if (bytesWritten >= 0) return new ImageflowException(message ?? "Empty error and stacktrace");

            if (defaultBufferSize < MaxBufferSize)
            {
                return FromContext(c, fullPaths, MaxBufferSize);
            }
            throw new ImageflowAssertionFailed(
                $"Imageflow error and stacktrace exceeded {MaxBufferSize} bytes");

        }
    }
}
