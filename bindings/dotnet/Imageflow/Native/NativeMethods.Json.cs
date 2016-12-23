using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using Newtonsoft.Json;

namespace Imageflow.Native
{
	internal static partial class NativeMethods
	{
		/// <summary>
		///		Writes fields from the given imageflow_json_response to the locations referenced.
		/// </summary>
		/// <param name="context"></param>
		/// <param name="response_in"></param>
		/// <param name="status_code_out"></param>
		/// <param name="buffer"></param>
		/// <returns></returns>
		/// <exception cref="BufferOverflowException"></exception>
		public static bool imageflow_json_response_read(ContextPointer context, JsonResponsePointer response_in, out long status_code_out, out JsonReader buffer)
		{
			IntPtr bufferPointer;
			IntPtr bufferSize;
			bool result = imageflow_json_response_read(context, response_in, out status_code_out, out bufferPointer, out bufferSize);
			if (!result)
			{
				buffer = null;
				return false;
			}
#pragma warning disable HeapAnalyzerExplicitNewObjectRule // Explicit new reference type allocation
			buffer = new UnmanagedJsonReader(bufferPointer, bufferSize.ToInt64());
#pragma warning restore HeapAnalyzerExplicitNewObjectRule // Explicit new reference type allocation
			return true;
		}

		/// <summary>
		///		Writes fields from the given imageflow_json_response to the locations referenced.
		/// </summary>
		/// <param name="context"></param>
		/// <param name="response_in"></param>
		/// <param name="status_code_out"></param>
		/// <param name="buffer_utf8_no_nulls_out"></param>
		/// <param name="buffer_size_out"></param>
		/// <returns></returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_json_response_read.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_json_response_read))]
		[return: MarshalAs(UnmanagedType.I1)]
		private static extern bool imageflow_json_response_read(ContextPointer context, JsonResponsePointer response_in, out long status_code_out, [Out]out IntPtr buffer_utf8_no_nulls_out, [Out]out IntPtr buffer_size_out);

		/// <summary>
		///		Frees memory associated with the given object (and owned objects) after running any owned or attached destructors.
		/// </summary>
		/// <param name="context"></param>
		/// <param name="response"></param>
		/// <returns><see langword="true"/> if something went wrong during tear-down. <see langword="true"/> if the object to destroy is a null pointer, or if tear-down was successful.</returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_json_response_destroy.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_json_response_destroy))]
		[return: MarshalAs(UnmanagedType.I1)]
		public static extern bool imageflow_json_response_destroy(ContextPointer context, JsonResponsePointer response);

		/// <summary>
		///		Sends a JSON message to the imageflow_context
		/// </summary>
		/// <param name="context"></param>
		/// <param name="method">Determines which code path will be used to process the provided JSON data and compose a response.</param>
		/// <param name="json"></param>
		/// <returns><see cref="JsonResponsePointer.Zero"/> on failure.</returns>
		public static JsonResponsePointer imageflow_context_send_json(ContextPointer context, string method, string json)
		{
			byte[] json_buffer = Encoding.UTF8.GetBytes(json);
			return imageflow_context_send_json(context, method, json_buffer, new IntPtr(json_buffer.Length));
		}

		/// <summary>
		///		Sends a JSON message to the imageflow_context
		/// </summary>
		/// <param name="context"></param>
		/// <param name="method">Determines which code path will be used to process the provided JSON data and compose a response.</param>
		/// <param name="json_buffer"></param>
		/// <param name="json_buffer_size"></param>
		/// <returns><see cref="JsonResponsePointer.Zero"/> on failure.</returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_context_send_json.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_context_send_json))]
		private static extern JsonResponsePointer imageflow_context_send_json(ContextPointer context, [MarshalAs(UnmanagedType.LPStr)][In]string method, [MarshalAs(UnmanagedType.LPArray, SizeParamIndex = 3)][In]byte[] json_buffer, IntPtr json_buffer_size);
	}
}
