using System;

namespace imageflow
{
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
