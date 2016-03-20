using System;
using System.Runtime.InteropServices;

namespace LibGD
{
    public class C
    {
        // FILE *fopen( const char *filename, const char *mode );
        [DllImport("msvcrt", CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        [return: MarshalAs(UnmanagedType.SysInt)]
        public static extern IntPtr fopen([In, MarshalAs(UnmanagedType.LPStr)] string filename, [In, MarshalAs(UnmanagedType.LPStr)] string mode);

        // int fclose( FILE *stream );
        [DllImport("msvcrt", CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
        [return: MarshalAs(UnmanagedType.I4)]
        public static extern int fclose([In] IntPtr stream);
    }

    /// <summary>
    /// This class, and more precisely, its internal struct, only serves debugging purposes.
    /// </summary>
    public unsafe partial class _iobuf
    {
        [StructLayout(LayoutKind.Sequential)]
        public struct Internal
        {
            public char* _ptr;
            public int _cnt;
            public char* _base;
            public int _flag;
            public int _file;
            public int _charbuf;
            public int _bufsiz;
            public char* _tmpfname;
        }
    }
}
