using System;
using System.Runtime.InteropServices;

namespace Imageflow.Native
{
	/// <summary>
	///		Pointer for a Job.
	/// </summary>
	/// <seealso cref="IEquatable{T}"/>
	/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow_core/struct.Job.html"/>
	[StructLayout(LayoutKind.Sequential)]
	internal struct JobPointer : IEquatable<JobPointer>
	{
		/// <summary>
		///		The job pointer
		/// </summary>
		private readonly IntPtr jobPointer;

		/// <summary>
		///		Initializes a new instance of the <see cref="JobPointer"/> struct.
		/// </summary>
		/// <param name="pointer">The pointer.</param>
		public JobPointer(IntPtr pointer)
		{
			jobPointer = pointer;
		}

		/// <summary>
		///		Indicates whether the current <see cref="JobPointer"/> is equal to another <see cref="JobPointer"/>.
		/// </summary>
		/// <param name="other">A <see cref="JobPointer"/> to compare with this <see cref="JobPointer"/>.</param>
		/// <returns>
		///		<see langword="true"/> if the current <see cref="JobPointer"/> is equal to the <paramref name="other" /> parameter; otherwise, <see langword="false"/>.
		/// </returns>
		public bool Equals(JobPointer other) => jobPointer == other.jobPointer;

		/// <summary>
		///		Determines whether the specified <see cref="object" />, is equal to this <see cref="JobPointer"/>.
		/// </summary>
		/// <param name="obj">The <see cref="object" /> to compare with this <see cref="JobPointer"/>.</param>
		/// <returns>
		///		<see langword="true"/> if the specified <see cref="object" /> is equal to this <see cref="JobPointer"/>; otherwise, <see langword="false"/>.
		/// </returns>
		public override bool Equals(object obj)
		{
			if (obj is JobPointer)
			{
				return Equals((JobPointer)obj);
			}
			return false;
		}

		/// <summary>
		///		Returns a hash code for this <see cref="JobPointer"/>.
		/// </summary>
		/// <returns>
		///		A hash code for this <see cref="JobPointer"/>, suitable for use in hashing algorithms and data structures like a hash table.
		/// </returns>
		public override int GetHashCode() => jobPointer.GetHashCode();

		/// <summary>
		///		Returns a <see cref="string" /> that represents this <see cref="JobPointer"/>.
		/// </summary>
		/// <returns>
		///		A <see cref="string" /> that represents this <see cref="JobPointer"/>.
		/// </returns>
		public override string ToString() => jobPointer.ToString();

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
		public static JobPointer Zero { get; } = new JobPointer(IntPtr.Zero);

		public static bool operator ==(JobPointer left, JobPointer right) => left.jobPointer == right.jobPointer;

		public static bool operator !=(JobPointer left, JobPointer right) => left.jobPointer != right.jobPointer;
	}
}