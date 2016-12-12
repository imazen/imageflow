using System;
using System.Runtime.InteropServices;

namespace Imageflow.Native
{
	/// <summary>
	///		Pointer for an Imageflow Context;
	/// </summary>
	/// <seealso cref="System.IEquatable{T}" />
	/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/struct.Context.html"/>
	[StructLayout(LayoutKind.Sequential)]
	internal struct ContextPointer : IEquatable<ContextPointer>
	{
		/// <summary>
		///		The internal pointer.
		/// </summary>
		private readonly IntPtr contextPointer;

		/// <summary>
		///		Initializes a new instance of the <see cref="ContextPointer"/> struct.
		/// </summary>
		/// <param name="pointer">The pointer.</param>
		public ContextPointer(IntPtr pointer)
		{
			contextPointer = pointer;
		}

		/// <summary>
		///		Indicates whether the current <see cref="ContextPointer"/> is equal to another <see cref="ContextPointer"/>.
		/// </summary>
		/// <param name="other">A <see cref="ContextPointer"/> to compare with this <see cref="ContextPointer"/>.</param>
		/// <returns>
		///		<see langword="true"/> if the current <see cref="ContextPointer"/> is equal to the <paramref name="other" /> parameter; otherwise, <see langword="false"/>.
		/// </returns>
		public bool Equals(ContextPointer other) => contextPointer == other.contextPointer;

		/// <summary>
		///		Determines whether the specified <see cref="object" />, is equal to this <see cref="ContextPointer"/>.
		/// </summary>
		/// <param name="obj">The <see cref="object" /> to compare with this <see cref="ContextPointer"/>.</param>
		/// <returns>
		///		<see langword="true"/> if the specified <see cref="object" /> is equal to this <see cref="ContextPointer"/>; otherwise, <see langword="false"/>.
		/// </returns>
		public override bool Equals(object obj)
		{
			if (obj is ContextPointer)
			{
				return Equals((ContextPointer)obj);
			}
			return false;
		}

		/// <summary>
		///		Returns a hash code for this <see cref="ContextPointer"/>.
		/// </summary>
		/// <returns>
		///		A hash code for this <see cref="ContextPointer"/>, suitable for use in hashing algorithms and data structures like a hash table.
		/// </returns>
		public override int GetHashCode() => contextPointer.GetHashCode();

		/// <summary>
		///		Returns a <see cref="string" /> that represents this <see cref="ContextPointer"/>.
		/// </summary>
		/// <returns>
		///		A <see cref="string" /> that represents this <see cref="ContextPointer"/>.
		/// </returns>
		public override string ToString() => contextPointer.ToString();

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
		public static ContextPointer Zero { get; } = new ContextPointer(IntPtr.Zero);

		public static bool operator ==(ContextPointer left, ContextPointer right) => left.contextPointer == right.contextPointer;

		public static bool operator !=(ContextPointer left, ContextPointer right) => left.contextPointer != right.contextPointer;
	}
}