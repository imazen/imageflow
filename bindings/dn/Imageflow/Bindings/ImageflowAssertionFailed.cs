using System;

namespace Imageflow.Bindings
{
    /// <inheritdoc />
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
