using System;
using System.Runtime.InteropServices;

namespace Imageflow.Native
{
	/// <summary>
	///		Pointer to memory allocated by Imageflow.
	/// </summary>
	/// <seealso cref="System.IEquatable{T}" />
	[StructLayout(LayoutKind.Sequential)]
	internal struct InternalMemoryPointer : IEquatable<InternalMemoryPointer>
	{
		/// <summary>
		///		The internal pointer
		/// </summary>
		private readonly IntPtr internalMemoryPointer;

		/// <summary>
		///		Initializes a new instance of the <see cref="InternalMemoryPointer"/> struct.
		/// </summary>
		/// <param name="pointer">The pointer.</param>
		public InternalMemoryPointer(IntPtr pointer)
		{
			internalMemoryPointer = pointer;
		}

		/// <summary>
		///		Indicates whether the current <see cref="InternalMemoryPointer"/> is equal to another <see cref="InternalMemoryPointer"/>.
		/// </summary>
		/// <param name="other">An <see cref="InternalMemoryPointer"/> to compare with this <see cref="InternalMemoryPointer"/>.</param>
		/// <returns>
		///		<see langword="true"/> if the current <see cref="InternalMemoryPointer"/> is equal to the <paramref name="other" /> parameter; otherwise, <see langword="false"/>.
		/// </returns>
		public bool Equals(InternalMemoryPointer other) => internalMemoryPointer == other.internalMemoryPointer;

		/// <summary>
		///		Determines whether the specified <see cref="object" />, is equal to this <see cref="InternalMemoryPointer"/>.
		/// </summary>
		/// <param name="obj">The <see cref="object" /> to compare with this <see cref="InternalMemoryPointer"/>.</param>
		/// <returns>
		///   <see langword="true"/> if the specified <see cref="object" /> is equal to this <see cref="InternalMemoryPointer"/>; otherwise, <see langword="false"/>.
		/// </returns>
		public override bool Equals(object obj)
		{
			if (obj is InternalMemoryPointer)
			{
				return Equals((InternalMemoryPointer)obj);
			}
			return false;
		}

		/// <summary>
		///		Returns a hash code for this <see cref="InternalMemoryPointer"/>.
		/// </summary>
		/// <returns>
		///		A hash code for this <see cref="InternalMemoryPointer"/>, suitable for use in hashing algorithms and data structures like a hash table.
		/// </returns>
		public override int GetHashCode() => internalMemoryPointer.GetHashCode();

		/// <summary>
		///		Returns a <see cref="string" /> that represents this <see cref="InternalMemoryPointer"/>.
		/// </summary>
		/// <returns>
		///		A <see cref="string" /> that represents this <see cref="InternalMemoryPointer"/>.
		/// </returns>
		public override string ToString() => internalMemoryPointer.ToString();

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
		public static InternalMemoryPointer Zero { get; } = new InternalMemoryPointer(IntPtr.Zero);

		public static bool operator ==(InternalMemoryPointer left, InternalMemoryPointer right) => left.internalMemoryPointer == right.internalMemoryPointer;

		public static bool operator !=(InternalMemoryPointer left, InternalMemoryPointer right) => left.internalMemoryPointer != right.internalMemoryPointer;
	}
}