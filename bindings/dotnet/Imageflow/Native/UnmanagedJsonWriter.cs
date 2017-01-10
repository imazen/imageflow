using System;
using System.IO;
using System.Text;
using Newtonsoft.Json;

namespace Imageflow.Native
{
	internal class UnmanagedJsonWriter : JsonWriter
	{
		private readonly JsonTextWriter jsonTextWriter;
		private readonly StreamWriter streamWriter;
		private readonly UnmanagedMemoryStream unmanagedMemoryStream;

		public UnmanagedJsonWriter(IntPtr pointer, long length)
		{
#pragma warning disable HeapAnalyzerExplicitNewObjectRule // Explicit new reference type allocation
			unsafe
			{
				unmanagedMemoryStream = new UnmanagedMemoryStream((byte*)pointer.ToPointer(), length, length, FileAccess.Write);
			}
			streamWriter = new StreamWriter(unmanagedMemoryStream, Encoding.UTF8, 1024, true);
			jsonTextWriter = new JsonTextWriter(streamWriter);
#pragma warning restore HeapAnalyzerExplicitNewObjectRule // Explicit new reference type allocation
		}

		public override void Flush()
		{
			jsonTextWriter.Flush();
			streamWriter.Flush();
		}

		public override void Close() => jsonTextWriter.Close();

		protected override void Dispose(bool disposing)
		{
			try
			{
				if (disposing)
				{
					(jsonTextWriter as IDisposable)?.Dispose();
					streamWriter?.Dispose();
					unmanagedMemoryStream?.Dispose();
				}
			}
			finally
			{
				base.Dispose(disposing);
			}
		}

		public override string ToString() => jsonTextWriter.ToString();

		public override void WriteComment(string text) => jsonTextWriter.WriteComment(text);

		public override void WriteEnd() => jsonTextWriter.WriteEnd();

		public override void WriteEndArray() => jsonTextWriter.WriteEndArray();

		public override void WriteEndConstructor() => jsonTextWriter.WriteEndConstructor();

		public override void WriteEndObject() => jsonTextWriter.WriteEndObject();

		public override void WriteNull() => jsonTextWriter.WriteNull();

		public override void WritePropertyName(string name) => jsonTextWriter.WritePropertyName(name);

		public override void WritePropertyName(string name, bool escape) => jsonTextWriter.WritePropertyName(name, escape);

		public override void WriteRaw(string json) => jsonTextWriter.WriteRaw(json);

		public override void WriteRawValue(string json) => jsonTextWriter.WriteRawValue(json);

		public override void WriteStartArray() => jsonTextWriter.WriteStartArray();

		public override void WriteStartConstructor(string name) => jsonTextWriter.WriteStartConstructor(name);

		public override void WriteStartObject() => jsonTextWriter.WriteStartObject();

		public override void WriteUndefined() => jsonTextWriter.WriteUndefined();

		public override void WriteValue(bool value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(bool? value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(byte value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(byte? value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(byte[] value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(char value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(char? value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(DateTime value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(DateTime? value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(DateTimeOffset value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(DateTimeOffset? value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(decimal value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(decimal? value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(double value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(double? value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(float value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(float? value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(Guid value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(Guid? value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(int value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(int? value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(long value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(long? value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(object value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(sbyte value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(sbyte? value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(short value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(short? value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(string value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(TimeSpan value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(TimeSpan? value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(uint value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(uint? value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(ulong value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(ulong? value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(Uri value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(ushort value) => jsonTextWriter.WriteValue(value);

		public override void WriteValue(ushort? value) => jsonTextWriter.WriteValue(value);

		public override void WriteWhitespace(string ws) => jsonTextWriter.WriteWhitespace(ws);
	}
}
