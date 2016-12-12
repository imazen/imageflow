using System;
using System.Runtime.InteropServices;

namespace Imageflow.Native
{
	internal static partial class NativeMethods
	{
		/// <summary>
		///		Creates a <see cref="JobIoPointer"/> object to wrap a filename.
		/// </summary>
		/// <param name="context"></param>
		/// <param name="mode"></param>
		/// <param name="filename"></param>
		/// <param name="cleanup"></param>
		/// <returns></returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_io_create_for_file.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_io_create_for_file))]
		public static extern JobIoPointer imageflow_io_create_for_file(ContextPointer context, IoMode mode, [MarshalAs(UnmanagedType.LPStr)]string filename, CleanupWith cleanup);

		/// <summary>
		///		Creates a <see cref="JobIoPointer"/> for reading from the provided buffer.
		/// </summary>
		/// <param name="context"></param>
		/// <param name="buffer"></param>
		/// <param name="buffer_byte_count"></param>
		/// <param name="lifetime">If you specify <see cref="Lifetime.OutlivesFunctionCall"/>, then the buffer will be copied.</param>
		/// <param name="cleanup"></param>
		/// <returns></returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_io_create_from_buffer.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_io_create_from_buffer))]
		public static extern JobIoPointer imageflow_io_create_from_buffer(ContextPointer context, [MarshalAs(UnmanagedType.LPArray, SizeParamIndex = 2)][In]byte[] buffer, IntPtr buffer_byte_count, Lifetime lifetime, CleanupWith cleanup);

		/// <summary>
		///		Creates a <see cref="JobIoPointer"/> for writing to an expanding memory buffer.
		/// </summary>
		/// <param name="context"></param>
		/// <returns><see cref="JobIoPointer.Zero"/> on failure.</returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_io_create_for_output_buffer.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_io_create_for_output_buffer))]
		public static extern JobIoPointer imageflow_io_create_for_output_buffer(ContextPointer context);

		/// <summary>
		///		Provides access to the underlying buffer for the given <see cref="JobIoPointer"/>.
		/// </summary>
		/// <param name="context"></param>
		/// <param name="io"></param>
		/// <param name="result_buffer"></param>
		/// <param name="result_buffer_length"></param>
		/// <returns></returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_io_get_output_buffer.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_io_get_output_buffer))]
		[return: MarshalAs(UnmanagedType.I1)]
		public static extern bool imageflow_io_get_output_buffer(ContextPointer context, JobIoPointer io, [MarshalAs(UnmanagedType.LPArray, SizeParamIndex = 3)][Out]out byte[] result_buffer, [Out]out long result_buffer_length);
	}
}
