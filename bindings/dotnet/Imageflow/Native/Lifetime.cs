namespace Imageflow.Native
{
	/// <summary>
	///		How long the provided pointer/buffer will remain valid. Callers must prevent the memory from being freed or moved until this contract expires.
	/// </summary>
	/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/enum.Lifetime.html"/>
	internal enum Lifetime
	{
		/// <summary>
		///		Pointer will outlive function call.
		/// </summary>
		OutlivesFunctionCall = 0,
		/// <summary>
		///		Pointer will outlive <see cref="ContextPointer"/>.
		/// </summary>
		OutlivesContext = 1,
	}
}