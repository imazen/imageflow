using System;

namespace Imageflow
{
	/// <summary>
	///		The exception that is thrown when a native buffer is too large to be marshaled into a managed byte array.
	/// </summary>
	/// <seealso cref="Exception" />
	public class BufferOverflowException : Exception
	{
		/// <summary>
		///		Initializes a new instance of the <see cref="BufferOverflowException"/> class.
		/// </summary>
		/// <param name="bufferSize">Size of the buffer.</param>
		public BufferOverflowException(long bufferSize)
		{
			BufferSize = bufferSize;
		}

		/// <summary>
		///		Initializes a new instance of the <see cref="BufferOverflowException"/> class.
		/// </summary>
		/// <param name="message">The message.</param>
		/// <param name="bufferSize">Size of the buffer.</param>
		public BufferOverflowException(string message, long bufferSize) : base(message)
		{
			BufferSize = bufferSize;
		}

		/// <summary>
		///		Initializes a new instance of the <see cref="BufferOverflowException"/> class.
		/// </summary>
		/// <param name="message">The message.</param>
		/// <param name="bufferSize">Size of the buffer.</param>
		/// <param name="innerException">The inner exception.</param>
		public BufferOverflowException(string message, long bufferSize, Exception innerException) : base(message, innerException)
		{
			BufferSize = bufferSize;
		}

		/// <summary>
		///		Gets the size of the buffer.
		/// </summary>
		/// <value>
		///		The size of the buffer.
		/// </value>
		public long BufferSize
		{
			get;
		}

		/// <summary>
		///		Gets a message that describes the current exception.
		/// </summary>
		public override string Message => $"The native buffer was too large to be marshaled into a managed byte array. Native Buffer Size: {BufferSize}.";
	}
}
