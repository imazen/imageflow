namespace Imageflow.Native
{
	/// <summary>
	///		When a resource should be closed/freed/cleaned up
	/// </summary>
	/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/enum.CleanupWith.html"/>
	internal enum CleanupWith
	{
		/// <summary>
		///		When the context is destroyed
		/// </summary>
		Context = 0,
		/// <summary>
		///		When the first job that the item is associated with is destroyed.
		/// </summary>
		FirstJob = 1,
	}
}