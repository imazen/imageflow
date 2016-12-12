using System;
using System.Runtime.InteropServices;

namespace Imageflow.Native
{
	/// <summary>
	///		Pointer for a JobIO.
	/// </summary>
	/// <seealso cref="IEquatable{T}" />
	[StructLayout(LayoutKind.Sequential)]
	internal struct JobIoPointer : IEquatable<JobIoPointer>
	{
		/// <summary>
		///		The JobIO pointer
		/// </summary>
		private readonly IntPtr jobIOPointer;

		/// <summary>
		///		Initializes a new instance of the <see cref="JobIoPointer"/> struct.
		/// </summary>
		/// <param name="pointer">The pointer.</param>
		public JobIoPointer(IntPtr pointer)
		{
			jobIOPointer = pointer;
		}

		/// <summary>
		///		Indicates whether the current <see cref="JobIoPointer"/> is equal to another <see cref="JobIoPointer"/>.
		/// </summary>
		/// <param name="other">A <see cref="JobIoPointer"/> to compare with this <see cref="JobIoPointer"/>.</param>
		/// <returns>
		///		<see langword="true"/> if the current <see cref="JobIoPointer"/> is equal to the <paramref name="other" /> parameter; otherwise, <see langword="false"/>.
		/// </returns>
		public bool Equals(JobIoPointer other) => jobIOPointer == other.jobIOPointer;

		/// <summary>
		///		Determines whether the specified <see cref="object" />, is equal to this <see cref="JobIoPointer"/>.
		/// </summary>
		/// <param name="obj">The <see cref="object" /> to compare with this <see cref="JobIoPointer"/>.</param>
		/// <returns>
		///		<see langword="true"/> if the specified <see cref="object" /> is equal to this <see cref="JobIoPointer"/>; otherwise, <see langword="false"/>.
		/// </returns>
		public override bool Equals(object obj)
		{
			if (obj is JobIoPointer)
			{
				return Equals((JobIoPointer)obj);
			}
			return false;
		}

		/// <summary>
		///		Returns a hash code for this <see cref="JobIoPointer"/>.
		/// </summary>
		/// <returns>
		///		A hash code for this <see cref="JobIoPointer"/>, suitable for use in hashing algorithms and data structures like a hash table.
		/// </returns>
		public override int GetHashCode() => jobIOPointer.GetHashCode();

		/// <summary>
		///		Returns a <see cref="string" /> that represents this <see cref="JobIoPointer"/>.
		/// </summary>
		/// <returns>
		///		A <see cref="string" /> that represents this <see cref="JobIoPointer"/>.
		/// </returns>
		public override string ToString() => jobIOPointer.ToString();

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
		public static JobIoPointer Zero { get; } = new JobIoPointer(IntPtr.Zero);

		public static bool operator ==(JobIoPointer left, JobIoPointer right) => left.jobIOPointer == right.jobIOPointer;

		public static bool operator !=(JobIoPointer left, JobIoPointer right) => left.jobIOPointer != right.jobIOPointer;
	}
}