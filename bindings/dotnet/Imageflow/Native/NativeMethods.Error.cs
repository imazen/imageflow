using System;
using System.Runtime.InteropServices;
using System.Text;

namespace Imageflow.Native
{
	internal static partial class NativeMethods
	{
		/// <summary>
		///
		/// </summary>
		/// <param name="context"></param>
		/// <returns>Returns true if the context is in an error state.</returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_context_has_error.html"/>
		/// <remarks>
		///		You must immediately deal with the error, as subsequent API calls will fail or cause undefined behavior until the error state is cleared.
		/// </remarks>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_context_has_error))]
		[return: MarshalAs(UnmanagedType.I1)]
		public static extern bool imageflow_context_has_error(ContextPointer context);

		/// <summary>
		///		Clear the error state.
		/// </summary>
		/// <param name="context"></param>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_context_clear_error.html"/>
		/// <remarks>
		///		This assumes that you know which API call failed and the problem has been resolved. Don't use this unless you're sure you've accounted for all possible inconsistent state (and fully understand the code paths that led to the error).
		/// </remarks>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_context_clear_error))]
		public static extern void imageflow_context_clear_error(ContextPointer context);

		/// <summary>
		///		Prints the error messages and stacktrace to the given buffer in UTF-8 form.
		/// </summary>
		/// <param name="context"></param>
		/// <param name="buffer"></param>
		/// <param name="buffer_length"></param>
		/// <param name="full_file_path"></param>
		/// <returns></returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_context_error_and_stacktrace.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_context_error_and_stacktrace))]
		public static extern NegativeInt32False imageflow_context_error_and_stacktrace(ContextPointer context, [MarshalAs(UnmanagedType.LPStr)][In][Out]StringBuilder buffer, IntPtr buffer_length, [MarshalAs(UnmanagedType.I1)]bool full_file_path);

		/// <summary>
		///		Returns the numeric code associated with the error.
		/// </summary>
		/// <param name="context"></param>
		/// <returns></returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_context_error_code.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_context_error_code))]
		public static extern ErrorInt32 imageflow_context_error_code(ContextPointer context);

		/// <summary>
		///		Prints the error to stderr and exits the process if an error has been raised on the context.
		/// </summary>
		/// <param name="context"></param>
		/// <returns>If no error is present, the function returns false.</returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_context_print_and_exit_if_error.html"/>
		/// <remarks>
		///		THIS PRINTS DIRECTLY TO STDERR! Do not use in any kind of service! Command-line usage only!
		/// </remarks>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_context_print_and_exit_if_error))]
		[return: MarshalAs(UnmanagedType.I1)]
		public static extern bool imageflow_context_print_and_exit_if_error(ContextPointer context);

		/// <summary>
		///		Raises an error on the context.
		/// </summary>
		/// <param name="context"></param>
		/// <param name="error_code"></param>
		/// <param name="message"></param>
		/// <param name="file"></param>
		/// <param name="line"></param>
		/// <param name="function_name"></param>
		/// <returns>Returns <see langword="true"/> on success, <see langword="false"/> if an error was already present.</returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_context_raise_error.html"/>
		/// <remarks>
		///		You cannot raise a second error until the first has been cleared with <see cref="imageflow_context_clear_error(ContextPointer)"/>. You'll be ignored, as will future <see cref="imageflow_context_add_to_callstack(ContextPointer, string, int, string)"/> invocations.
		/// </remarks>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_context_raise_error))]
		[return: MarshalAs(UnmanagedType.I1)]
		public static extern bool imageflow_context_raise_error(ContextPointer context, ErrorInt32 error_code, [MarshalAs(UnmanagedType.LPStr)][In]string message, [MarshalAs(UnmanagedType.LPStr)][In]string file = null, int line = -1, [MarshalAs(UnmanagedType.LPStr)][In]string function_name = null);

		/// <summary>
		///		Adds the given filename, line number, and function name to the call stack.
		/// </summary>
		/// <param name="context"></param>
		/// <param name="filename"></param>
		/// <param name="line"></param>
		/// <param name="function_name"></param>
		/// <returns><see langword="true"/> if add was successful; otherwise <see langword="false"/>.</returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_context_add_to_callstack.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_context_add_to_callstack))]
		[return: MarshalAs(UnmanagedType.I1)]
		public static extern bool imageflow_context_add_to_callstack(ContextPointer context, [MarshalAs(UnmanagedType.LPStr)][In]string filename = null, int line = -1, [MarshalAs(UnmanagedType.LPStr)][In]string function_name = null);
	}
}
