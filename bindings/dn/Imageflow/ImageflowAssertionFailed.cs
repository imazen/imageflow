using System;

namespace Imageflow
{
    /// <summary>
    /// For bugs
    /// </summary>
    public class ImageflowAssertionFailed : Exception
    {
        public ImageflowAssertionFailed(string message) : base(message)
        {
            
        }
    }
}
