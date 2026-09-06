using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading;
using Imageflow.Bindings;
using Imageflow.Fluent;

static class Program
{
    static int _logged, _loggedPrinted;
    static void LogResult(string s)
    {
        Interlocked.Increment(ref _logged);
        if (Interlocked.Increment(ref _loggedPrinted) <= 2) Console.WriteLine("LOG: " + (s.Length > 140 ? s.Substring(0, 140).Replace("\n", " | ") + "..." : s));
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

    static int Main(string[] args)
    {
        // Usage: Repro737.exe <img1[;img2;...]> <iterations-per-thread> [stackKb] [qs] [threads]
        string[] paths = args[0].Split(';');
        int n = int.Parse(args[1]);
        int stackKb = args.Length > 2 ? int.Parse(args[2]) : 0;
        string qstr = args.Length > 3 ? args[3] : "width=100&mode=max";
        int threads = args.Length > 4 ? int.Parse(args[4]) : 1;
        Console.WriteLine($"Process: {(Environment.Is64BitProcess ? "x64" : "x86")}, CLR {Environment.Version}, stack={(stackKb > 0 ? stackKb + " KB" : "default")}, qs={qstr}, threads={threads}, images={paths.Length}");
        AppDomain.CurrentDomain.UnhandledException += (s, e) => Console.WriteLine("UNHANDLED: " + e.ExceptionObject);
        int nulls = 0, oks = 0;
        Action<int> body = (tid) =>
        {
            for (int i = 1; i <= n; i++)
            {
                var path = paths[(i + tid) % paths.Length];
                var r = GetImageBytes(path, qstr);
                if (r == null) Interlocked.Increment(ref nulls); else Interlocked.Increment(ref oks);
                if (tid == 0 && (i <= 3 || i % 25 == 0))
                {
                    GC.Collect(); GC.WaitForPendingFinalizers(); GC.Collect();
                    var p = Process.GetCurrentProcess();
                    Console.WriteLine($"t0 iter {i}: nulls={Volatile.Read(ref nulls)} oks={Volatile.Read(ref oks)} private={p.PrivateMemorySize64 / 1024} KB handles={p.HandleCount} threads={p.Threads.Count}");
                }
            }
        };
        var ts = Enumerable.Range(0, threads).Select(tid => stackKb > 0 ? new Thread(() => body(tid), stackKb * 1024) : new Thread(() => body(tid))).ToList();
        ts.ForEach(t => t.Start());
        ts.ForEach(t => t.Join());
        Console.WriteLine($"DONE logged={_logged} nulls={nulls} oks={oks}");
        return 0;
    }
}
