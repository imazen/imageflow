using System;

namespace Imageflow
{
    public static class ExtensionMethods
    {
        public static bool IsZero(this IntPtr p) => p == IntPtr.Zero;
    }
    
}
