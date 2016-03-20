using System;
using System.IO;

namespace LibGD
{
    public partial class gd
    {
        public static gdImageStruct gdImageCreateFromPng(byte[] bytes)
        {
            return ReadFromByteArray(bytes, gdImageCreateFromPngPtr);
        }

        public static gdImageStruct gdImageCreateFromGif(byte[] bytes)
        {
            return ReadFromByteArray(bytes, gdImageCreateFromGifPtr);
        }

        public static gdImageStruct gdImageCreateFromWBMP(byte[] bytes)
        {
            return ReadFromByteArray(bytes, gdImageCreateFromWBMPPtr);
        }

        public static gdImageStruct gdImageCreateFromJpeg(byte[] bytes)
        {
            return ReadFromByteArray(bytes, gdImageCreateFromJpegPtr);
        }

        public static unsafe gdImageStruct gdImageCreateFromJpegEx(byte[] bytes, int ignore_warning)
        {
            fixed (byte* data = bytes)
            {
                return gdImageCreateFromJpegPtrEx(bytes.Length, new IntPtr(data), ignore_warning);
            }
        }

#if !NO_TIFF

        public static gdImageStruct gdImageCreateFromTiff(byte[] bytes)
        {
            return ReadFromByteArray(bytes, gdImageCreateFromTiffPtr);
        }

#endif

        public static gdImageStruct gdImageCreateFromTga(byte[] bytes)
        {
            return ReadFromByteArray(bytes, gdImageCreateFromTgaPtr);
        }

        public static gdImageStruct gdImageCreateFromBmp(byte[] bytes)
        {
            return ReadFromByteArray(bytes, gdImageCreateFromBmpPtr);
        }

        public static gdImageStruct gdImageCreateFromGd(byte[] bytes)
        {
            return ReadFromByteArray(bytes, gdImageCreateFromGdPtr);
        }

        public static gdImageStruct gdImageCreateFromGd2(byte[] bytes)
        {
            return ReadFromByteArray(bytes, gdImageCreateFromGd2Ptr);
        }

        public static unsafe gdImageStruct gdImageCreateFromGd2Part(byte[] bytes, int srcx, int srcy, int w, int h)
        {
            fixed (byte* data = bytes)
            {
                return gdImageCreateFromGd2PartPtr(bytes.Length, new IntPtr(data), srcx, srcy, w, h);
            }
        }

        public static gdImageStruct gdImageCreateFromXbm(byte[] bytes)
        {
            string temp = Path.GetTempFileName();
            File.WriteAllBytes(temp, bytes);
            try
            {
                return gdImageCreateFromXbm(temp);
            }
            finally
            {
                File.Delete(temp);
            }
        }

        public static gdImageStruct gdImageCreateFromPng(string file)
        {
            return ReadFromFile(file, gdImageCreateFromPng);
        }

        public static gdImageStruct gdImageCreateFromGif(string file)
        {
            return ReadFromFile(file, gdImageCreateFromGif);
        }

        public static gdImageStruct gdImageCreateFromWBMP(string file)
        {
            return ReadFromFile(file, gdImageCreateFromWBMP);
        }

        public static gdImageStruct gdImageCreateFromJpeg(string file)
        {
            return ReadFromFile(file, gdImageCreateFromJpeg);
        }

        public static gdImageStruct gdImageCreateFromJpegEx(string file, int ignore_warning)
        {
            IntPtr fd = C.fopen(file ?? string.Empty, "rb");
            try
            {
                return gdImageCreateFromJpegEx(fd, ignore_warning);
            }
            finally
            {
                Close(fd);
            }
        }

#if !NO_TIFF

        public static gdImageStruct gdImageCreateFromTiff(string file)
        {
            return ReadFromFile(file, gdImageCreateFromTiff);
        }

#endif

        public static gdImageStruct gdImageCreateFromTga(string file)
        {
            return ReadFromFile(file, gdImageCreateFromTga);
        }

        public static gdImageStruct gdImageCreateFromBmp(string file)
        {
            return ReadFromFile(file, gdImageCreateFromBmp);
        }

        public static gdImageStruct gdImageCreateFromGd(string file)
        {
            return ReadFromFile(file, gdImageCreateFromGd);
        }

        public static gdImageStruct gdImageCreateFromGd2(string file)
        {
            return ReadFromFile(file, gdImageCreateFromGd2);
        }

        public static gdImageStruct gdImageCreateFromGd2Part(string file, int srcx, int srcy, int w, int h)
        {
            IntPtr fd = C.fopen(file ?? string.Empty, "rb");
            try
            {
                return gdImageCreateFromGd2Part(fd, srcx, srcy, w, h);
            }
            finally
            {
                Close(fd);
            }
        }

        public static gdImageStruct gdImageCreateFromXbm(string file)
        {
            return ReadFromFile(file, gdImageCreateFromXbm);
        }

        public static gdImageStruct gdImageCreateFromPng(Stream stream)
        {
            return ReadFromStream(stream, gdImageCreateFromPng);
        }

        public static gdImageStruct gdImageCreateFromGif(Stream stream)
        {
            return ReadFromStream(stream, gdImageCreateFromGif);
        }

        public static gdImageStruct gdImageCreateFromWBMP(Stream stream)
        {
            return ReadFromStream(stream, gdImageCreateFromWBMP);
        }

        public static gdImageStruct gdImageCreateFromJpeg(Stream stream)
        {
            return ReadFromStream(stream, gdImageCreateFromJpeg);
        }

        public static gdImageStruct gdImageCreateFromJpegEx(Stream stream, int ignore_warning)
        {
            using (var output = new MemoryStream())
            {
                stream.CopyTo(output);
                return gdImageCreateFromJpegEx(output.ToArray(), ignore_warning);
            }
        }

#if !NO_TIFF

        public static gdImageStruct gdImageCreateFromTiff(Stream stream)
        {
            return ReadFromStream(stream, gdImageCreateFromTiff);
        }

#endif

        public static gdImageStruct gdImageCreateFromTga(Stream stream)
        {
            return ReadFromStream(stream, gdImageCreateFromTga);
        }

        public static gdImageStruct gdImageCreateFromBmp(Stream stream)
        {
            return ReadFromStream(stream, gdImageCreateFromBmp);
        }

        public static gdImageStruct gdImageCreateFromGd(Stream stream)
        {
            return ReadFromStream(stream, gdImageCreateFromGd);
        }

        public static gdImageStruct gdImageCreateFromGd2(Stream stream)
        {
            return ReadFromStream(stream, gdImageCreateFromGd2);
        }

        public static gdImageStruct gdImageCreateFromGd2Part(Stream stream, int srcx, int srcy, int w, int h)
        {
            using (var output = new MemoryStream())
            {
                stream.CopyTo(output);
                return gdImageCreateFromGd2Part(output.ToArray(), srcx, srcy, w, h);
            }
        }

        public static gdImageStruct gdImageCreateFromXbm(Stream stream)
        {
            using (var output = new MemoryStream())
            {
                stream.CopyTo(output);
                return gdImageCreateFromXbm(output.ToArray());
            }
        }

        private static unsafe gdImageStruct ReadFromByteArray(byte[] bytes, Func<int, IntPtr, gdImageStruct> function)
        {
            fixed (byte* data = bytes)
            {
                return function(bytes.Length, new IntPtr(data));
            }
        }

        private static gdImageStruct ReadFromFile(string file, Func<IntPtr, gdImageStruct> function)
        {
            IntPtr fd = C.fopen(file ?? string.Empty, "rb");
            try
            {
                return function(fd);
            }
            finally
            {
                Close(fd);
            }
        }

        private static gdImageStruct ReadFromStream(Stream stream, Func<byte[], gdImageStruct> function)
        {
            using (var output = new MemoryStream())
            {
                stream.CopyTo(output);
                return function(output.ToArray());
            }
        }


        public static void gdImageBmp(gdImageStruct im, string outFile, int compression)
        {
            var fp = C.fopen(outFile ?? string.Empty, "wb");
            try
            {
                gd.gdImageBmp(im, fp, 1);
            }
            finally
            {
                Close(fp);
            }
        }

        public static void gdImageGd(gdImageStruct im, string outFile)
        {
            var fp = C.fopen(outFile ?? string.Empty, "wb");
            try
            {
                gd.gdImageGd(im, fp);
            }
            finally
            {
                Close(fp);
            }
        }

        public static void gdImageGd2(gdImageStruct im, string @out, int cs, int fmt)
        {
            var fp = C.fopen(@out ?? string.Empty, "wb");
            try
            {
                gd.gdImageGd2(im, fp, cs, fmt);
            }
            finally
            {
                Close(fp);
            }
        }

        public static void gdImagePng(gdImageStruct im, string outFile)
        {
            var fp = C.fopen(outFile ?? string.Empty, "wb");
            try
            {
                gd.gdImagePng(im, fp);
            }
            finally
            {
                Close(fp);
            }
        }

        public static void gdImageJpeg(gdImageStruct im, string @out, int quality)
        {
            var fp = C.fopen(@out ?? string.Empty, "wb");
            try
            {
                gd.gdImageJpeg(im, fp, quality);
            }
            finally
            {
                Close(fp);
            }
        }

        public static void gdImageGif(gdImageStruct im, string outFile)
        {
            var fp = C.fopen(outFile ?? string.Empty, "wb");
            try
            {
                gd.gdImageGif(im, fp);
            }
            finally
            {
                Close(fp);
            }
        }

#if !NO_TIFF

        public static void gdImageTiff(gdImageStruct im, string outFile)
        {
            var fp = C.fopen(outFile ?? string.Empty, "wb");
            try
            {
                gd.gdImageTiff(im, fp);
            }
            finally
            {
                Close(fp);
            }
        }

#endif

        private static void Close(IntPtr fp)
        {
            if (fp != IntPtr.Zero)
            {
                C.fclose(fp);
            }
        }
    }

    namespace GD
    {
        public unsafe partial class Image
        {
            public void Gd(string @out)
            {
                SaveToFile(@out, Gd);
            }

            public void Gd2(string @out, int cs, int fmt)
            {
                var fp = C.fopen(@out ?? string.Empty, "wb");
                try
                {
                    Gd2(fp, cs, fmt);
                }
                finally
                {
                    Close(fp);
                }
            }

            public bool CreateFromGd(string @in)
            {
                return CreateFromFile(@in, CreateFromGd);
            }

            public bool CreateFromGd2(string @in)
            {
                return CreateFromFile(@in, CreateFromGd2);
            }

            public bool CreateFromPng(string @in)
            {
                return CreateFromFile(@in, CreateFromPng);
            }

            public bool CreateFromJpeg(string @in)
            {
                return CreateFromFile(@in, CreateFromJpeg);
            }

            public bool CreateFromGif(string @in)
            {
                return CreateFromFile(@in, CreateFromGif);
            }

            public bool CreateFromGd(byte[] bytes)
            {
                return CreateFromBytes(bytes, CreateFromGd);
            }

            public bool CreateFromGd2(byte[] bytes)
            {
                return CreateFromBytes(bytes, CreateFromGd2);
            }

            public bool CreateFromPng(byte[] bytes)
            {
                return CreateFromBytes(bytes, CreateFromPng);
            }

            public bool CreateFromJpeg(byte[] bytes)
            {
                return CreateFromBytes(bytes, CreateFromJpeg);
            }

            public bool CreateFromGif(byte[] bytes)
            {
                return CreateFromBytes(bytes, CreateFromGif);
            }

            public bool CreateFromGd(Stream stream)
            {
                return CreateFromStream(stream, CreateFromGd);
            }

            public bool CreateFromGd2(Stream stream)
            {
                return CreateFromStream(stream, CreateFromGd2);
            }

            public bool CreateFromPng(Stream stream)
            {
                return CreateFromStream(stream, CreateFromPng);
            }

            public bool CreateFromJpeg(Stream stream)
            {
                return CreateFromStream(stream, CreateFromJpeg);
            }

            public bool CreateFromGif(Stream stream)
            {
                return CreateFromStream(stream, CreateFromGif);
            }

            public void Png(string @out)
            {
                SaveToFile(@out, Png);
            }

            public void Jpeg(string @out, int quality = -1)
            {
                var fp = C.fopen(@out ?? string.Empty, "wb");
                try
                {
                    Jpeg(fp, quality);
                }
                finally
                {
                    Close(fp);
                }
            }

            public void Gif(string @out)
            {
                SaveToFile(@out, Gif);
            }

            private static bool CreateFromFile(string @in, Func<IntPtr, bool> function)
            {
                var fp = C.fopen(@in ?? string.Empty, "rb");
                try
                {
                    return function(fp);
                }
                finally
                {
                    Close(fp);
                }
            }

            private static bool CreateFromBytes(byte[] bytes, Func<int, IntPtr, bool> function)
            {
                fixed (byte* data = bytes)
                {
                    return function(bytes.Length, new IntPtr(data));
                }
            }

            private static bool CreateFromStream(Stream stream, Func<byte[], bool> function)
            {
                using (var output = new MemoryStream())
                {
                    stream.CopyTo(output);
                    return function(output.ToArray());
                }
            }

            private static void SaveToFile(string @out, Action<IntPtr> action)
            {
                var fp = C.fopen(@out ?? string.Empty, "wb");
                try
                {
                    action(fp);
                }
                finally
                {
                    Close(fp);
                }
            }

            private static void Close(IntPtr fp)
            {
                if (fp != IntPtr.Zero)
                {
                    C.fclose(fp);
                }
            }
        }
    }
}
