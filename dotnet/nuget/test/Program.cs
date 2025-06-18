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
        Console.WriteLine("================================================");
        Console.WriteLine($"Test program has started running for RID: {System.Runtime.InteropServices.RuntimeInformation.RuntimeIdentifier}");

        var imageBytes = Convert.FromBase64String(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABAQMAAAAl21bKAAAAA1BMVEX/TQBcNTh/AAAAAXRSTlPM0jRW/QAAAApJREFUeJxjYgAAAAYAAzY3fKgAAAAASUVORK5CYII=");

        try
        {
        
            var versionInfo = new Imageflow.Bindings.JobContext().GetVersionInfo();
            Console.WriteLine($"Loaded imageflow dynamic library version {versionInfo.LongVersionString }");
            
            var info = await ImageJob.GetImageInfoAsync(new MemorySource(imageBytes), SourceLifetime.NowOwnedAndDisposedByTask);
            Console.WriteLine("Decoded tiny PNG: GetImageInfoAsync returned ImageWidth: {0}", info.ImageWidth );


            Console.WriteLine("\u2705 Test program has successfully used imageflow library.");
            Console.WriteLine("================================================");
            //Environment.ExitCode = 0; // Explicit success
            return 0;
        }
        catch (Exception ex)
        {
            Console.WriteLine($"\u274C Test program has failed: {ex.Message}");
            Console.WriteLine(ex.StackTrace); // Provide more details on error
            Console.WriteLine("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
            //Environment.ExitCode = 1; // Explicit failure
            return 1;
        }
    }
}
