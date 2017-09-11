
using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Security;
using System.Text;
using System.Threading;

namespace Imageflow.Bindings
{
    interface ILibraryLoadLogger
    {
        void NotifyAttempt(string basename, string fullPath, bool fileExists, bool previouslyLoaded, int? loadErrorCode);
    }
    internal static class NativeLibraryLoader
    {
        static bool IsUnix => Environment.OSVersion.Platform == PlatformID.Unix || Environment.OSVersion.Platform == PlatformID.MacOSX;

        static readonly Lazy<string> SharedLibraryPrefix = new Lazy<string>(() => IsUnix ? "lib" : "", LazyThreadSafetyMode.PublicationOnly);

        static readonly Lazy<bool> IsDotNetCore = new Lazy<bool>(() =>
            typeof(System.Runtime.GCSettings).GetTypeInfo().Assembly.CodeBase.Contains("Microsoft.NETCore.App")
            , LazyThreadSafetyMode.PublicationOnly);

        static readonly Lazy<string> SharedLibraryExtension = new Lazy<string>(() =>
        {
            if (Environment.OSVersion.Platform == PlatformID.MacOSX)
                return "dylib";
            if (Environment.OSVersion.Platform == PlatformID.Unix)
                return "so";
            return "dll";
        }, LazyThreadSafetyMode.PublicationOnly);

        /// <summary>
        /// The output subdirectory that NuGet .props/.targets should be copying unmanaged binaries to.
        /// If you're using .NET Core you don't need this.
        /// </summary>
        static readonly Lazy<string> ArchitectureSubdir = new Lazy<string>(() =>
        {
            if (!IsUnix)
            {
                var architecture = Environment.GetEnvironmentVariable("PROCESSOR_ARCHITECTURE");
                if (string.Equals(architecture, "ia64", StringComparison.OrdinalIgnoreCase))
                {
                    return "ia64";
                }
                else if (string.Equals(architecture, "arm", StringComparison.OrdinalIgnoreCase))
                {
                    return Environment.Is64BitProcess ? "arm64" : "arm";
                }
                // We don't currently support unlisted/unknown architectures. We default to x86/x64 as backup
            }
            return Environment.Is64BitProcess ? "x64" : "x86";
        }, LazyThreadSafetyMode.PublicationOnly);

        static private IEnumerable<string> BaseFolders(IEnumerable<string> customSearchDirectories = null)
        {
            // Prioritize user suggestions
            if (customSearchDirectories != null)
            {
                foreach (string d in customSearchDirectories)
                {
                    yield return d;
                }
            }
            // Look where .NET looks for managed assemblies
            yield return AppDomain.CurrentDomain.BaseDirectory;
            // Look in the folder that *this* assembly is located.
            yield return Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location);
        }

        static private IEnumerable<string> SearchPossibilitiesForFile(string filename, IEnumerable<string> customSearchDirectories = null)
        {
            var subdir = ArchitectureSubdir.Value;
            foreach (string f in BaseFolders(customSearchDirectories))
            {
                if (string.IsNullOrEmpty(f)) continue;
                var directory = Path.GetFullPath(f);
                // Try architecture-specific subdirectories first
                if (subdir != null)
                {
                    yield return Path.Combine(directory, subdir, filename);
                }
                // Try the folder itself
                yield return Path.Combine(directory, filename);
            }
        }

        static bool LoadLibrary(string fullPath, out IntPtr handle, out int? errorCode)
        {
            handle = IsUnix ? UnixLoadLibrary.Execute(fullPath) : WindowsLoadLibrary.Execute(fullPath);
            if (handle == IntPtr.Zero)
            {
                errorCode = Marshal.GetLastWin32Error();
                return false;
            }
            else
            {
                errorCode = null;
                return true;
            }
        }

        private static bool TryLoadByBasenameInternal(string basename, ILibraryLoadLogger log, out IntPtr handle, IEnumerable<string> customSearchDirectories = null)
        {
            var filename = $"{SharedLibraryPrefix.Value}{basename}.{SharedLibraryExtension.Value}";

            foreach (string path in SearchPossibilitiesForFile(filename, customSearchDirectories))
            {
                if (!File.Exists(path))
                {
                    log.NotifyAttempt(basename, path, false, false, 0);
                }
                else
                {
                    var success = LoadLibrary(path, out handle, out var errorCode);
                    log.NotifyAttempt(basename, path, true, false, errorCode);
                    if (success)
                    {
                        return true;
                    }
                    else
                    {
                        continue;
                    }
                }
            }
            handle = IntPtr.Zero;
            return false;
        }

        static readonly Lazy<ConcurrentDictionary<string, IntPtr>> LibraryHandlesByBasename = new Lazy<ConcurrentDictionary<string, IntPtr>>(() => new ConcurrentDictionary<string, IntPtr>(StringComparer.OrdinalIgnoreCase), LazyThreadSafetyMode.PublicationOnly);

        // Not yet implemented. 
        // static readonly Lazy<ConcurrentDictionary<string, IntPtr>> LibraryHandlesByFullPath = new Lazy<ConcurrentDictionary<string, IntPtr>>(() => new ConcurrentDictionary<string, IntPtr>(StringComparer.OrdinalIgnoreCase), LazyThreadSafetyMode.PublicationOnly);

        /// <summary>
        /// Searches known directories for the provided file basename (or returns true if one is already loaded)
        /// basename 'imageflow' -> imageflow.dll, libimageflow.so, libimageflow.dylib.
        /// Basename is case-sensitive
        /// </summary>
        /// <param name="basename">The library name sans extension or "lib" prefix</param>
        /// <param name="log">Where to log attempts at assembly search and load</param>
        /// <param name="handle">Where to store the loaded library handle</param>
        /// <param name="customSearchDirectory">Provide this if you want a custom search folder</param>
        /// <returns>True if previously or successfully loaded</returns>
        public static bool TryLoadByBasename(string basename, ILibraryLoadLogger log, out IntPtr handle, IEnumerable<string> customSearchDirectories = null)
        {
            if (string.IsNullOrEmpty(basename))
                throw new ArgumentNullException("filenameWithoutExtension");

            if (LibraryHandlesByBasename.Value.TryGetValue(basename, out handle))
            {
                log.NotifyAttempt(basename, null, true, true, 0);
                return true;
            }
            else
            {
                lock (LibraryHandlesByBasename)
                {
                    if (LibraryHandlesByBasename.Value.TryGetValue(basename, out handle))
                    {
                        log.NotifyAttempt(basename, null, true, true, 0);
                        return true;
                    }
                    else
                    {
                        var success = TryLoadByBasenameInternal(basename, log, out handle, customSearchDirectories);
                        if (success) LibraryHandlesByBasename.Value[basename] = handle;
                        return success;
                    }
                }
            }
        }


        class LoadLogger : ILibraryLoadLogger
        {
            internal Exception firstException;
            internal Exception lastException;

            List<LogEntry> log = new List<LogEntry>(7);
            struct LogEntry
            {
                internal string basename;
                internal string fullPath;
                internal bool fileExists;
                internal bool previouslyLoaded;
                internal int? loadErrorCode;
            }
            public void NotifyAttempt(string basename, string fullPath, bool fileExists, bool previouslyLoaded, int? loadErrorCode)
            {
                log.Add(new LogEntry { basename = basename,
                    fullPath = fullPath,
                    fileExists = fileExists,
                    previouslyLoaded = previouslyLoaded,
                    loadErrorCode = loadErrorCode });
            }

            internal void RaiseException()
            {
                var sb = new StringBuilder(log.Select((e) => e.basename?.Length ?? 0 + e.fullPath?.Length ?? 0 + 20).Sum());
                sb.AppendFormat("Using \"{0}[basename].{1}\" Subdir=\"{2}\", IsUnix={3}, IsDotNetCore={4}\n", SharedLibraryPrefix.Value, SharedLibraryExtension.Value, ArchitectureSubdir.Value, IsUnix, IsDotNetCore.Value);
                if (firstException != null) sb.AppendFormat("Before searching: {0}\n", firstException.Message);
                foreach (var e in log)
                {
                    if (e.previouslyLoaded)
                    {
                        sb.AppendFormat("\"{0}\" is already loaded", e.basename);
                    }
                    else if (!e.fileExists)
                    {
                        sb.AppendFormat("File not found: {0}", e.fullPath);
                    }
                    else if (e.loadErrorCode.HasValue)
                    {
                        sb.AppendFormat("Error {0} loading {1} from {2}", new Win32Exception(e.loadErrorCode.Value).Message, e.basename, e.fullPath);
                    }
                    else
                    {
                        sb.AppendFormat("Loaded {0} from {1}", e.basename, e.fullPath);
                    }
                    sb.Append('\n');
                }
                if (lastException != null) sb.AppendLine(lastException.Message);
                var stackTrace = (firstException ?? lastException)?.StackTrace;
                if (stackTrace != null) sb.AppendLine(stackTrace);

                throw new DllNotFoundException(sb.ToString());
            }
        }

        /// <summary>
        /// Attempts to resolve DllNotFoundException
        /// </summary>
        /// <typeparam name="T"></typeparam>
        /// <param name="basename"></param>
        /// <param name="invokingOperation"></param>
        /// <returns></returns>
        public static T FixDllNotFoundException<T>(string basename, Func<T> invokingOperation, IEnumerable<string> customSearchDirectories = null)
        {
            // It turns out that trying to do it "before" is 4-5x slower in cases where the standard loading mechanism works
            // And catching the DllNotFoundException does not seem to measurably slow us down. So no "preventative" stuff.
            try
            {
                return invokingOperation();
            }
            catch (DllNotFoundException first)
            {
                var logger = new LoadLogger();
                logger.firstException = first;
                if (TryLoadByBasename("imageflow", logger, out var handle, customSearchDirectories))
                {
                    try
                    {
                        return invokingOperation();
                    }
                    catch (DllNotFoundException last)
                    {
                        logger.lastException = last;
                    }
                }
                logger.RaiseException();
            }
            return default(T);
        }
    }

    [SuppressUnmanagedCodeSecurity]
    [SecurityCritical]
    static class WindowsLoadLibrary
    {
        [DllImport("kernel32", CallingConvention = CallingConvention.Winapi, CharSet = CharSet.Unicode, SetLastError = true)]
        static extern IntPtr LoadLibraryEx(string fileName, IntPtr reservedNull, uint flags);

        public static IntPtr Execute(string fileName)
        {
            // Look in the library dir instead of the process dir 
            const uint LOAD_WITH_ALTERED_SEARCH_PATH = 0x00000008;
            return LoadLibraryEx(fileName, IntPtr.Zero, LOAD_WITH_ALTERED_SEARCH_PATH);
        }
    }

    [SuppressUnmanagedCodeSecurity]
    [SecurityCritical]
    static class UnixLoadLibrary
    {
        // TODO: unsure if this works on Mac OS X; it might be libc instead
        [DllImport("libdl.so", SetLastError = true)]
        static extern IntPtr dlopen(String fileName, int flags);

        public static IntPtr Execute(string fileName)
        {
            const int RTLD_NOW = 2;
            return dlopen(fileName, RTLD_NOW);
        }
    }

    
}
