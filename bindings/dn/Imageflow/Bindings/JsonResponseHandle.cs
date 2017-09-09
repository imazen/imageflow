using System;
using System.Runtime.ConstrainedExecution;
using Microsoft.Win32.SafeHandles;

namespace Imageflow.Bindings
{
    internal sealed class JsonResponseHandle : SafeHandleZeroOrMinusOneIsInvalid, IAssertReady
    {
        private readonly JobContextHandle _parent;

        public JsonResponseHandle(JobContextHandle parent, IntPtr ptr)
            : base(true)
        {
            _parent = parent ?? throw new ArgumentNullException("parent");
            SetHandle(ptr);

        }

        public JobContextHandle ParentContext => _parent;

        public bool IsValid => !IsInvalid && !IsClosed && _parent.IsValid;

        public void AssertReady()
        {
            if (!_parent.IsValid) throw new ObjectDisposedException("Imageflow JobContextHandle");
            if (!IsValid) throw new ObjectDisposedException("Imageflow JsonResponseHandle");
        }
        
        public ImageflowException DisposeAllowingException()
        {
            ImageflowException e = null;
            if (IsValid)
            {
                try
                {
                    if (!NativeMethods.imageflow_json_response_destroy(_parent, DangerousGetHandle()))
                    {
                        e = ImageflowException.FromContext(_parent);
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
            return !_parent.IsValid || NativeMethods.imageflow_json_response_destroy(_parent, handle);
        }
    }
}