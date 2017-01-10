using System;
using System.IO;
using System.Text;
using Newtonsoft.Json;

namespace Imageflow.Native
{
	/// <summary>
	///		A reader that provides fast, non-cached, forward-only access to serialized JSON data in unmanaged memory.
	/// </summary>
	internal class UnmanagedJsonReader : JsonReader
	{
		/// <summary>
		///		The <see cref="JsonTextReader"/> doing the actual reading.
		/// </summary>
		private readonly JsonTextReader jsonTextReader;
		/// <summary>
		///		A <see cref="StreamReader"/> bridging <see cref="unmanagedMemoryStream"/> and <see cref="jsonTextReader"/>.
		/// </summary>
		private readonly StreamReader streamReader;
		/// <summary>
		///		Provides access to unmanaged memory.
		/// </summary>
		private readonly UnmanagedMemoryStream unmanagedMemoryStream;

		/// <summary>
		///		Initializes a new instance of the <see cref="UnmanagedJsonReader"/> class.
		/// </summary>
		/// <param name="pointer">The pointer to the unmanaged memory.</param>
		/// <param name="length">The length of the memory section to read.</param>
		public UnmanagedJsonReader(IntPtr pointer, long length)
		{
#pragma warning disable HeapAnalyzerExplicitNewObjectRule // Explicit new reference type allocation
			unsafe
			{
				unmanagedMemoryStream = new UnmanagedMemoryStream((byte*)pointer.ToPointer(), length, length, FileAccess.Read);
			}
			streamReader = new StreamReader(unmanagedMemoryStream, Encoding.UTF8, false, 1024, true);
			jsonTextReader = new JsonTextReader(streamReader);
#pragma warning restore HeapAnalyzerExplicitNewObjectRule // Explicit new reference type allocation
		}

		/// <summary>
		///		Gets the depth of the current token in the JSON document.
		/// </summary>
		/// <value>
		///		The depth of the current token in the JSON document.
		/// </value>
		public override int Depth => jsonTextReader.Depth;

		/// <summary>
		///		Gets the path of the current JSON token.
		/// </summary>
		/// <value>
		///		The path of the current JSON token.
		/// </value>
		public override string Path => jsonTextReader.Path;

		/// <summary>
		///		Gets the type of the current JSON token.
		/// </summary>
		/// <value>
		///		The type of the current JSON token.
		/// </value>
		public override JsonToken TokenType => jsonTextReader.TokenType;

		/// <summary>
		///		Gets the text value of the current JSON token.
		/// </summary>
		/// <value>
		///		The text value of the current JSON token.
		/// </value>
		public override object Value => jsonTextReader.Value;

		/// <summary>
		///		Gets the Common Language Runtime (CLR) type for the current JSON token.
		/// </summary>
		/// <value>
		///		The Common Language Runtime (CLR) type for the current JSON token.
		/// </value>
		public override Type ValueType => jsonTextReader.ValueType;

		/// <summary>
		///		Changes <see cref="JsonReader.CurrentState" /> to <see cref="JsonReader.State.Closed"/>.
		/// </summary>
		public override void Close() => jsonTextReader.Close();

		/// <summary>
		///		Reads the next JSON token from the stream.
		/// </summary>
		/// <returns>
		///		<see langword="true"/> if the next token was read successfully; <see langword="false"/> if there are no more tokens to read.
		/// </returns>
		public override bool Read() => jsonTextReader.Read();

		/// <summary>
		///		Reads the next JSON token from the stream as a <see cref="bool" /><c>?</c>.
		/// </summary>
		/// <returns>
		///		A <see cref="bool" /><c>?</c>. This method will return <see langword="null"/> at the end of an array.
		/// </returns>
		public override bool? ReadAsBoolean() => jsonTextReader.ReadAsBoolean();

		/// <summary>
		///		Reads the next JSON token from the stream as a <see cref="byte" /><c>[]</c>.
		/// </summary>
		/// <returns>
		///		A <see cref="byte" /><c>[]</c> or a <see langword="null"/> reference if the next JSON token is <see langword="null"/>. This method will return <see langword="null"/> at the end of an array.
		/// </returns>
		public override byte[] ReadAsBytes() => jsonTextReader.ReadAsBytes();

		/// <summary>
		///		Reads the next JSON token from the stream as a <see cref="DateTime" /><c>?</c>.
		/// </summary>
		/// <returns>
		///		A <see cref="DateTime" /><c>?</c>. This method will return <see langword="null"/> at the end of an array.
		/// </returns>
		public override DateTime? ReadAsDateTime() => jsonTextReader.ReadAsDateTime();

		/// <summary>
		///		Reads the next JSON token from the stream as a <see cref="DateTimeOffset" /><c>?</c>.
		/// </summary>
		/// <returns>
		///		A <see cref="DateTimeOffset" /><c>?</c>. This method will return <see langword="null"/> at the end of an array.
		/// </returns>
		public override DateTimeOffset? ReadAsDateTimeOffset() => jsonTextReader.ReadAsDateTimeOffset();

		/// <summary>
		///		Reads the next JSON token from the stream as a <see cref="decimal" /><c>?</c>.
		/// </summary>
		/// <returns>
		///		A <see cref="decimal" /><c>?</c>. This method will return <see langword="null"/> at the end of an array.
		/// </returns>
		public override decimal? ReadAsDecimal() => jsonTextReader.ReadAsDecimal();

		/// <summary>
		///		Reads the next JSON token from the stream as a <see cref="double" /><c>?</c>.
		/// </summary>
		/// <returns>
		///		A <see cref="double" /><c>?</c>. This method will return <see langword="null"/> at the end of an array.
		/// </returns>
		public override double? ReadAsDouble() => jsonTextReader.ReadAsDouble();

		/// <summary>
		///		Reads the next JSON token from the stream as a <see cref="int" /><c>?</c>.
		/// </summary>
		/// <returns>
		///		A <see cref="int" /><c>?</c>. This method will return <see langword="null"/> at the end of an array.
		/// </returns>
		public override int? ReadAsInt32() => jsonTextReader.ReadAsInt32();

		/// <summary>
		///		Reads the next JSON token from the stream as a <see cref="string" />.
		/// </summary>
		/// <returns>
		///		A <see cref="string" />. This method will return <see langword="null"/> at the end of an array.
		/// </returns>
		public override string ReadAsString() => jsonTextReader.ReadAsString();

		/// <summary>
		///		Releases unmanaged and - optionally - managed resources
		/// </summary>
		/// <param name="disposing"><see langword="true"/> to release both managed and unmanaged resources; <see langword="false"/> to release only unmanaged resources.</param>
		protected override void Dispose(bool disposing)
		{
			try
			{
				if (disposing)
				{
					(jsonTextReader as IDisposable)?.Dispose();
					streamReader?.Dispose();
					unmanagedMemoryStream?.Dispose();
				}
			}
			finally
			{
				base.Dispose(disposing);
			}
		}
	}
}
