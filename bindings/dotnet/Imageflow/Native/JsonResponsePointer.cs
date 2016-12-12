using System;
using System.Runtime.InteropServices;

namespace Imageflow.Native
{
	/// <summary>
	///		Response containing JSON.
	/// </summary>
	/// <seealso cref="System.IEquatable{T}" />
	/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/struct.JsonResponse.html"/>
	[StructLayout(LayoutKind.Sequential)]
	internal struct JsonResponsePointer : IEquatable<JsonResponsePointer>
	{
		/// <summary>
		///		The internal pointer.
		/// </summary>
		private readonly IntPtr jsonResponsePointer;

		/// <summary>
		///		Initializes a new instance of the <see cref="JsonResponsePointer"/> struct.
		/// </summary>
		/// <param name="pointer">The pointer.</param>
		public JsonResponsePointer(IntPtr pointer)
		{
			jsonResponsePointer = pointer;
		}

		/// <summary>
		///		Indicates whether the current <see cref="JsonResponsePointer"/> is equal to another <see cref="JsonResponsePointer"/>.
		/// </summary>
		/// <param name="other">A <see cref="JsonResponsePointer"/> to compare with this <see cref="JsonResponsePointer"/>.</param>
		/// <returns>
		///		<see langword="true"/> if the current <see cref="JsonResponsePointer"/> is equal to the <paramref name="other" /> parameter; otherwise, <see langword="false"/>.
		/// </returns>
		public bool Equals(JsonResponsePointer other) => jsonResponsePointer == other.jsonResponsePointer;

		/// <summary>
		///		Determines whether the specified <see cref="object" />, is equal to this <see cref="JsonResponsePointer"/>.
		/// </summary>
		/// <param name="obj">The <see cref="object" /> to compare with this <see cref="JsonResponsePointer"/>.</param>
		/// <returns>
		///   <see langword="true"/> if the specified <see cref="object" /> is equal to this <see cref="JsonResponsePointer"/>; otherwise, <see langword="false"/>.
		/// </returns>
		public override bool Equals(object obj)
		{
			if (obj is JsonResponsePointer)
			{
				return Equals((JsonResponsePointer)obj);
			}
			return false;
		}

		/// <summary>
		///		Returns a hash code for this <see cref="JsonResponsePointer"/>.
		/// </summary>
		/// <returns>
		///		A hash code for this <see cref="JsonResponsePointer"/>, suitable for use in hashing algorithms and data structures like a hash table.
		/// </returns>
		public override int GetHashCode() => jsonResponsePointer.GetHashCode();

		/// <summary>
		///		Returns a <see cref="string" /> that represents this <see cref="JsonResponsePointer"/>.
		/// </summary>
		/// <returns>
		///		A <see cref="string" /> that represents this <see cref="JsonResponsePointer"/>.
		/// </returns>
		public override string ToString() => jsonResponsePointer.ToString();

		/// <summary>
		///		Gets the size of this instance.
		/// </summary>
		/// <value>
		///		The size of this instance.
		/// </value>
		/// <seealso cref="IntPtr.Size"/>
		public static int Size => IntPtr.Size;

		/// <summary>
		///		Gets the <see langword="null"/> value.
		/// </summary>
		/// <value>
		///		The <see langword="null"/> value.
		/// </value>
		public static JsonResponsePointer Zero { get; } = new JsonResponsePointer(IntPtr.Zero);

		public static bool operator ==(JsonResponsePointer left, JsonResponsePointer right) => left.jsonResponsePointer == right.jsonResponsePointer;

		public static bool operator !=(JsonResponsePointer left, JsonResponsePointer right) => left.jsonResponsePointer != right.jsonResponsePointer;
	}
}