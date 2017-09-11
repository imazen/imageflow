using System;
using System.Runtime.ConstrainedExecution;
using Microsoft.Win32.SafeHandles;
using System.Diagnostics;

namespace Imageflow.Bindings
{
    
    /// <summary>
    /// The handle is ready even if there is an error condition stored in the context
    /// </summary>
    internal sealed class JobContextHandle : SafeHandleZeroOrMinusOneIsInvalid, IAssertReady
    {
        public JobContextHandle()
            : base(true)
        {
            //var timer = Stopwatch.StartNew();
            IntPtr ptr = NativeLibraryLoader.FixDllNotFoundException<IntPtr>("imageflow", () => NativeMethods.imageflow_context_create(NativeMethods.ABI_MAJOR, NativeMethods.ABI_MINOR));
            //timer.Stop();
            //Debug.WriteLine($"{timer.ElapsedMilliseconds}ms"); //4ms (when pinvoke 'just' works) to 27ms (when we have to go looking for the binary)
            if (ptr == IntPtr.Zero)
            {

                if (NativeMethods.imageflow_abi_compatible(NativeMethods.ABI_MAJOR, NativeMethods.ABI_MINOR))
                {
                    throw new OutOfMemoryException("Failed to create Imageflow JobContext");
                }
                var major = NativeMethods.imageflow_abi_version_major();
                var minor = NativeMethods.imageflow_abi_version_minor();
                throw new Exception(
                    $".NET Imageflow bindings only support ABI {NativeMethods.ABI_MAJOR}.{NativeMethods.ABI_MINOR}. libimageflow ABI {major}.{minor} is loaded.");
            }
            SetHandle(ptr);
        }

        public bool IsValid => !IsInvalid && !IsClosed;

        public void AssertReady()
        {
            if (!IsValid) throw new ObjectDisposedException("Imageflow JobContextHandle");
        }

        public ImageflowException DisposeAllowingException()
        {
            ImageflowException e = null;
            if (IsValid)
            {
                try
                {
                    if (!NativeMethods.imageflow_context_begin_terminate(this))
                    {
                        e = ImageflowException.FromContext(this);
                    }
                }
                finally
                {
                    Dispose();
                }
            }
            return e;
        }

        [ReliabilityContract(Consistency.WillNotCorruptState, Cer.Success)]
        protected override bool ReleaseHandle()
        {
            NativeMethods.imageflow_context_destroy(handle);
            return true;
        }
    }
}
