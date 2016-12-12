using System.Runtime.InteropServices;

namespace Imageflow.Native
{
	internal static partial class NativeMethods
	{
		private const string LibraryName = "imageflow";

		/// <summary>
		///		Creates and returns an imageflow context. An imageflow context is required for all other imageflow API calls.
		/// </summary>
		/// <returns>Returns a null pointer if allocation fails.</returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_context_create.html"/>
		/// <remarks>
		///		Contexts are not thread-safe! Once you create a context, you are responsible for ensuring that it is never involved in two overlapping API calls.
		/// </remarks>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_context_create))]
		public static extern ContextPointer imageflow_context_create();

		/// <summary>
		///		Begins the process of destroying the context, yet leaves error information intact so that any errors in the tear-down process can be debugged.
		/// </summary>
		/// <param name="context"></param>
		/// <returns>Returns true if no errors occurred. Returns false if there were tear-down issues.</returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_context_begin_terminate.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_context_begin_terminate))]
		[return: MarshalAs(UnmanagedType.I1)]
		public static extern bool imageflow_context_begin_terminate(ContextPointer context);

		/// <summary>
		///		Destroys the imageflow context and frees the context object.
		/// </summary>
		/// <param name="context"></param>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_context_destroy.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_context_destroy))]
		public static extern void imageflow_context_destroy(ContextPointer context);

		/// <summary>
		///		Allocates zeroed memory that will be freed with the context.
		/// </summary>
		/// <param name="context"></param>
		/// <param name="bytes"></param>
		/// <param name="filename"></param>
		/// <param name="line"></param>
		/// <returns><see cref="InternalMemoryPointer.Zero"/> on failure.</returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_context_memory_allocate.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_context_memory_allocate))]
		public static extern InternalMemoryPointer imageflow_context_memory_allocate(ContextPointer context, uint bytes, [MarshalAs(UnmanagedType.LPStr)]string filename = null, int line = -1);

		/// <summary>
		///		Frees memory allocated with <see cref="imageflow_context_memory_allocate(ContextPointer, uint, string, int)"/> early.
		/// </summary>
		/// <param name="context"></param>
		/// <param name="pointer"></param>
		/// <param name="filename"></param>
		/// <param name="line"></param>
		/// <returns><see langword="true"/> if freed; <see langword="false"/> on failure.</returns>
		/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_context_memory_free.html"/>
		[DllImport(LibraryName, EntryPoint = nameof(imageflow_context_memory_free))]
		[return: MarshalAs(UnmanagedType.I1)]
		public static extern bool imageflow_context_memory_free(ContextPointer context, InternalMemoryPointer pointer, [MarshalAs(UnmanagedType.LPStr)]string filename = null, int line = -1);
	}
}
