using System;

namespace Imageflow
{
    public class ImageflowAssertionFailed : Exception
    {
        public ImageflowAssertionFailed(string message) : base(message)
        {
            
        }
    }
}
