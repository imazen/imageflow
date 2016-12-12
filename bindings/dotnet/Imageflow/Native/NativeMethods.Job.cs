using System;
using System.Runtime.InteropServices;
using System.Text;

namespace Imageflow.Native
{
	internal static partial class NativeMethods
	{
		/// <summary>
		///		Sends a JSON message to the <see cref="JobPointer"/>.
		/// </summary>
		/// <param name="context">The context.</param>
		/// <param name="job">The job.</param>
		/// <param name="method">Determines which code path will be used to process the provided JSON data and compose a response.</param>
		/// <param name="json">The json.</param>
		/// <returns><see cref="JsonResponsePointer.Zero"/> on failure.</returns>
		public static JsonResponsePointer imageflow_job_send_json(ContextPointer context, JobPointer job, string method, string json)
		{
			byte[] json_buffer = Encoding.UTF8.GetBytes(json);
			return imageflow_job_send_json(context, job, method, json_buffer, new IntPtr(json_buffer.Length));
		}

		/// <summary>
		///		Sends a JSON message to the <see cref="JobPointer"/>.
		/// </summary>
		/// <param name="context"></param>
		/// <param name="job"></param>
		/// <param name="method">Determines which code path will be used to process the provided JSON data and compose a response.</param>
		/// <param name="json_buffer"></param>
		/// <param name="json_buffer_size"></param>
		/// <returns><see cref="JsonResponsePointer.Zero"/> on failure.</returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_job_send_json.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_job_send_json))]
		private static extern JsonResponsePointer imageflow_job_send_json(ContextPointer context, JobPointer job, [MarshalAs(UnmanagedType.LPStr)][In]string method, [MarshalAs(UnmanagedType.LPArray, SizeParamIndex = 4)][In]byte[] json_buffer, IntPtr json_buffer_size);

		/// <summary>
		///		Provides access to the underlying buffer for the given <see cref="JobPointer"/>.
		/// </summary>
		/// <param name="context"></param>
		/// <param name="job"></param>
		/// <param name="io_id"></param>
		/// <param name="result"></param>
		/// <returns></returns>
		/// <exception cref="BufferOverflowException"></exception>
		public static bool imageflow_job_get_output_buffer_by_id(ContextPointer context, JobPointer job, int io_id, out string result)
		{
			byte[] buffer;
			long bufferSize;
			bool @return = imageflow_job_get_output_buffer_by_id(context, job, io_id, out buffer, out bufferSize);
			if (!@return)
			{
				result = null;
				return false;
			}
			if (bufferSize > int.MaxValue)
			{
#pragma warning disable HeapAnalyzerExplicitNewObjectRule // Explicit new reference type allocation
				throw new BufferOverflowException(bufferSize);
#pragma warning restore HeapAnalyzerExplicitNewObjectRule // Explicit new reference type allocation
			}
			result = Encoding.UTF8.GetString(buffer, 0, (int)bufferSize);
			return true;
		}

		/// <summary>
		///		Provides access to the underlying buffer for the given <see cref="JobPointer"/>.
		/// </summary>
		/// <param name="context"></param>
		/// <param name="job"></param>
		/// <param name="io_id"></param>
		/// <param name="result_buffer"></param>
		/// <param name="result_buffer_length"></param>
		/// <returns></returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_job_get_output_buffer_by_id.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_job_get_output_buffer_by_id))]
		[return: MarshalAs(UnmanagedType.I1)]
		private static extern bool imageflow_job_get_output_buffer_by_id(ContextPointer context, JobPointer job, int io_id, [MarshalAs(UnmanagedType.LPArray, SizeParamIndex = 4)][Out]out byte[] result_buffer, [Out]out long result_buffer_length);

		/// <summary>
		///		Creates an imageflow_job, which permits the association of imageflow_io instances with numeric identifiers and provides a 'sub-context' for job execution.
		/// </summary>
		/// <param name="context"></param>
		/// <returns></returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_job_create.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_job_create))]
		public static extern JobPointer imageflow_job_create(ContextPointer context);

		/// <summary>
		///		Looks up the imageflow_io pointer from the provided io_id
		/// </summary>
		/// <param name="context"></param>
		/// <param name="job"></param>
		/// <param name="placeholder_id"></param>
		/// <returns></returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_job_get_io.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_job_get_io))]
		public static extern JobIoPointer imageflow_job_get_io(ContextPointer context, JobPointer job, int placeholder_id);

		/// <summary>
		///		Associates the imageflow_io object with the job and the assigned io_id.
		/// </summary>
		/// <param name="context"></param>
		/// <param name="job"></param>
		/// <param name="io"></param>
		/// <param name="placeholder_id"></param>
		/// <param name="direction"></param>
		/// <returns></returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_job_add_io.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_job_add_io))]
		[return: MarshalAs(UnmanagedType.I1)]
		public static extern bool imageflow_job_add_io(ContextPointer context, JobPointer job, IntPtr io, int placeholder_id, Direction direction);

		/// <summary>
		///		Destroys the provided imageflow_job.
		/// </summary>
		/// <param name="context"></param>
		/// <param name="job"></param>
		/// <returns></returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_job_destroy.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_job_destroy))]
		[return: MarshalAs(UnmanagedType.I1)]
		public static extern bool imageflow_job_destroy(ContextPointer context, JobPointer job);
	}
}
