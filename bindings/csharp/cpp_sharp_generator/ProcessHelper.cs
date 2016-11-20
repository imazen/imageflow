using System;
using System.Diagnostics;

namespace LibGD.CLI
{
    public class ProcessHelper
    {
        public static string Run(string path, string args, out string error)
        {
            try
            {
                using (var process = new Process())
                {
                    process.StartInfo.FileName = path;
                    process.StartInfo.Arguments = args;
                    process.StartInfo.UseShellExecute = false;
                    process.StartInfo.RedirectStandardOutput = true;
                    process.StartInfo.RedirectStandardError = true;
                    process.Start();
                    process.WaitForExit();
                    error = process.StandardError.ReadToEnd();
                    if (process.ExitCode != 0)
                    {
                        return string.Empty;
                    }
                    return process.StandardOutput.ReadToEnd().Trim().Replace(@"\\", @"\");
                }
            }
            catch (Exception exception)
            {
                error = string.Format("Calling {0} caused an exception: {1}.", path, exception.Message);
                return string.Empty;
            }
        }
    }
}
