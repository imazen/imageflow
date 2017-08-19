using System;

namespace imageflow
{
    // For bugs in user code, where they're trying to use something after disposing it
    public class ImageflowDisposedException : Exception
    {
        public ImageflowDisposedException(string disposedObject)
        {
            DisposedObject = disposedObject;
        }

        public string DisposedObject { get; }


        public override string Message => $"The Imageflow {DisposedObject} has been disposed and cannot be used.";
    }
}
