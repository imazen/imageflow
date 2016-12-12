namespace Imageflow.Native
{
	/// <summary>
	///		What is possible with the IO object
	/// </summary>
	/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/enum.IoMode.html"/>
	internal enum IoMode
	{
		None = 0,
		ReadSequential = 1,
		WriteSequential = 2,
		ReadSeekable = 5,
		WriteSeekable = 6,
		ReadWriteSeekable = 15,
	}
}