using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading;
using System.Web;
using Imageflow.Bindings;
using Imageflow.Fluent;

namespace Repro737Web
{
    public class ResizeHandler : IHttpHandler
    {
        public bool IsReusable => true;
        // Directory holding the test images; web-log.txt is written here too.
        static readonly string Dir = Environment.GetEnvironmentVariable("IMAGEFLOW_REPRO_IMAGE_DIR")
            ?? Path.Combine(HttpRuntime.AppDomainAppPath, "images");
        static int _count;
        static readonly object LogLock = new object();

        public void ProcessRequest(HttpContext context)
        {
            var img = Path.GetFileName(context.Request.QueryString["img"] ?? "cmyk_icc_on_rgb_1.jpg");
            var qs = context.Request.QueryString["qs"] ?? "width=100&mode=max";
            var bytes = GetImageBytes(Path.Combine(Dir, img), qs);
            var n = Interlocked.Increment(ref _count);
            if (context.Request.QueryString["gc"] == "1") { GC.Collect(); GC.WaitForPendingFinalizers(); GC.Collect(); }
            var p = Process.GetCurrentProcess();
            context.Response.ContentType = "text/plain";
            context.Response.Write($"{n} pid={p.Id} {(Environment.Is64BitProcess ? "x64" : "x86")} img={img} result={(bytes == null ? "null" : bytes.Length + " bytes")} private={p.PrivateMemorySize64 / 1024}KB handles={p.HandleCount} threads={p.Threads.Count}\n");
        }

        static void LogResult(string s)
        {
            lock (LogLock)
            {
                File.AppendAllText(Path.Combine(Dir, "web-log.txt"),
                    DateTime.UtcNow.ToString("o") + " pid=" + Process.GetCurrentProcess().Id + " " + s.Replace("\n", " | ").Substring(0, Math.Min(160, s.Length)) + "\n");
            }
        }

        // Verbatim from https://github.com/imazen/imageflow/issues/737#issuecomment-6597976600
        public static byte[] GetImageBytes(string filepath, string qstr)
        {
            byte[] imageBytes = null;

            try
            {
                imageBytes = File.ReadAllBytes(filepath);

                using (var b = new ImageJob())
                {
                    var r = b.BuildCommandString(
                        MemorySource.Borrow(imageBytes),
                        new BytesDestination(),
                        qstr
                    ).Finish().InProcessAsync().GetAwaiter().GetResult();

                    if (r != null && r.First != null)
                    {
                        var result = r.First.TryGetBytes();
                        if (result.HasValue)
                        {
                            return result.Value.ToArray();
                        }
                    }
                }
            }
            catch (ImageflowException ex)
            {
                // Bad/malformed image (e.g. CMYK profile on RGB data): log it, don't crash
               LogResult("Imageflow: " + ex.Message + "|" + filepath + "|" + qstr);
                return null; // caller decides: placeholder, 415, or original file
            }
            catch (Exception ex)
            {
                // Anything else (file missing, IO error, etc.)
                LogResult(ex.Message + "|" + filepath + "|" + qstr);
                return null;
            }

            return imageBytes;
        }
    }
}
