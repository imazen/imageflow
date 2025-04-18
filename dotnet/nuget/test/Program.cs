using System;
// Attempt to use a type from the referenced Imageflow.Net package
// Replace with an actual type from Imageflow.Net if known, otherwise this might need adjustment.
// For now, using a common namespace as a placeholder.
// using Imageflow.Net;
using Imageflow.Fluent;

class Program
{
    static async Task<int> Main(string[] args)
    {
        Console.WriteLine($"Test program running for RID: {System.Runtime.InteropServices.RuntimeInformation.RuntimeIdentifier}");

        var imageBytes = Convert.FromBase64String(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABAQMAAAAl21bKAAAAA1BMVEX/TQBcNTh/AAAAAXRSTlPM0jRW/QAAAApJREFUeJxjYgAAAAYAAzY3fKgAAAAASUVORK5CYII=");

        try
        {
            var info = await ImageJob.GetImageInfoAsync(new MemorySource(imageBytes), SourceLifetime.NowOwnedAndDisposedByTask);
            Console.WriteLine("ImageInfo: {0}", info);
            // Placeholder: Add code here that actually P/Invokes or uses Imageflow.Net 
            // to verify the correct native library is loaded and functional.
            // Example (conceptual):
            // var versionInfo = await new Imageflow.Net.JobContext().GetVersionAsync();
            // Console.WriteLine($"Successfully called Imageflow.Net. Got version info: {versionInfo.ImageflowBuildDetails}");

            Console.WriteLine("Test program completed successfully.");
            Environment.ExitCode = 0; // Explicit success
            return 0;
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Test program failing: {ex.Message}");
            Console.WriteLine(ex.StackTrace); // Provide more details on error
            Environment.ExitCode = 1; // Explicit failure
            return 1;
        }
    }
}
