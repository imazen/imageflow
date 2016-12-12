namespace Imageflow.Native
{
	/// <summary>
	///		Defined Error Codes
	/// </summary>
	/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_context_error_code.html"/>
	internal enum ImageflowError
	{
		/// <summary>
		///		No error condition.
		/// </summary>
		None = 0,
		/// <summary>
		///		Out Of Memory condition (malloc/calloc/realloc failed).
		/// </summary>
		OutOfMemory = 10,
		/// <summary>
		///		I/O error.
		/// </summary>
		IoError = 20,
		/// <summary>
		///		Invalid internal state (assertion failed; you found a bug).
		/// </summary>
		InvalidInternalState = 30,
		/// <summary>
		///		Error: Not implemented. (Feature not implemented).
		/// </summary>
		NotImplemented = 40,
		/// <summary>
		///		Invalid argument provided.
		/// </summary>
		InvalidArgument = 50,
		/// <summary>
		///		Null argument provided.
		/// </summary>
		NullArgument = 51,
		/// <summary>
		///		Invalid dimensions.
		/// </summary>
		InvalidDimensions = 52,
		/// <summary>
		///		Unsupported pixel format.
		/// </summary>
		PixelFormatUnsupported = 53,
		/// <summary>
		///		Item does not exist.
		/// </summary>
		ItemDoesNotExist = 54,
		/// <summary>
		///		Image decoding failed.
		/// </summary>
		DecodingImageFailed = 60,
		/// <summary>
		///		Image encoding failed.
		/// </summary>
		EncodingImageFailed = 61,
		/// <summary>
		///		Graph invalid.
		/// </summary>
		InvalidGraph = 70,
		/// <summary>
		///		Graph is cyclic.
		/// </summary>
		CyclicGraph = 71,
		/// <summary>
		///		Invalid inputs to node.
		/// </summary>
		InvalidNodeInput = 72,
		/// <summary>
		///		Maximum graph passes exceeded.
		/// </summary>
		ExceededMaximumGraphPasses = 73,
		/// <summary>
		///		Other error; something else happened.
		/// </summary>
		Other = 1024
	}
}