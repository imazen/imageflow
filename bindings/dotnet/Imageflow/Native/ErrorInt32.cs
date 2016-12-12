using System;
using System.Runtime.InteropServices;

namespace Imageflow.Native
{
	/// <summary>
	///		Error Integer
	/// </summary>
	/// <seealso cref="System.IComparable" />
	/// <seealso cref="System.IComparable{T}" />
	/// <seealso cref="System.IEquatable{T}" />
	/// <seealso cref="IFormattable" />
	/// <seealso cref="ImageflowError"/>
	/// <seealso href="https://s3-us-west-1.amazonaws.com/imageflow-nightlies/master/doc/imageflow/fn.imageflow_context_error_code.html"/>
	[StructLayout(LayoutKind.Sequential)]
	internal struct ErrorInt32 : IComparable, IComparable<int>, IEquatable<int>, IFormattable, IComparable<ErrorInt32>, IEquatable<ErrorInt32>
	{
		/// <summary>
		///		Wrapped <see cref="int"/>.
		/// </summary>
		private readonly int value;

		/// <summary>
		///		Initializes a new instance of the <see cref="ErrorInt32"/> struct.
		/// </summary>
		/// <param name="value">The value to wrap.</param>
		public ErrorInt32(int value)
		{
			this.value = value;
		}

		/// <summary>
		///		Compares the current <see cref="ErrorInt32"/> with another <see cref="ErrorInt32"/> and returns an integer that indicates whether the current <see cref="ErrorInt32"/> precedes, follows, or occurs in the same position in the sort order as the other <see cref="ErrorInt32"/>.
		/// </summary>
		/// <param name="other">An <see cref="ErrorInt32"/> to compare with this <see cref="ErrorInt32"/>.</param>
		/// <returns>
		///		A value that indicates the relative order of the <see cref="ErrorInt32"/>s being compared. The return value has these meanings:
		///		<list type="table">
		///			<listheader>
		///				<term>Value</term>
		///				<term>Meaning</term>
		///			</listheader>
		///			<item>
		///				<description>Less than zero</description>
		///				<description>This <see cref="ErrorInt32"/> precedes <paramref name="other" /> in the sort order.</description>
		///			</item>
		///			<item>
		///				<description>Zero</description>
		///				<description>This <see cref="ErrorInt32"/> occurs in the same position in the sort order as <paramref name="other" />.</description>
		///			</item>
		///			<item>
		///				<description>Greater than zero</description>
		///				<description>This <see cref="ErrorInt32"/> follows <paramref name="other" /> in the sort order.</description>
		///			</item>
		///		</list>
		/// </returns>
		public int CompareTo(ErrorInt32 other) => value.CompareTo(other.value);

		/// <summary>
		///		Compares the current <see cref="ErrorInt32"/> with an <see cref="int"/> and returns an integer that indicates whether the current <see cref="ErrorInt32"/> precedes, follows, or occurs in the same position in the sort order as the <see cref="int"/>.
		/// </summary>
		/// <param name="other">An <see cref="int"/> to compare with this <see cref="ErrorInt32"/>.</param>
		/// <returns>
		///		A value that indicates the relative order of the <see cref="ErrorInt32"/> and <see cref="int"/> being compared. The return value has these meanings:
		///		<list type="table">
		///			<listheader>
		///				<term>Value</term>
		///				<term>Meaning</term>
		///			</listheader>
		///			<item>
		///				<description>Less than zero</description>
		///				<description>This <see cref="ErrorInt32"/> precedes <paramref name="other" /> in the sort order.</description>
		///			</item>
		///			<item>
		///				<description>Zero</description>
		///				<description>This <see cref="ErrorInt32"/> occurs in the same position in the sort order as <paramref name="other" />.</description>
		///			</item>
		///			<item>
		///				<description>Greater than zero</description>
		///				<description>This <see cref="ErrorInt32"/> follows <paramref name="other" /> in the sort order.</description>
		///			</item>
		///		</list>
		/// </returns>
		public int CompareTo(int other) => value.CompareTo(other);

		/// <summary>
		///		Compares the current <see cref="ErrorInt32"/> with an <see cref="object"/> and returns an integer that indicates whether the current <see cref="ErrorInt32"/> precedes, follows, or occurs in the same position in the sort order as the <see cref="object"/>.
		/// </summary>
		/// <param name="obj">An <see cref="object"/> to compare with this <see cref="ErrorInt32"/>.</param>
		/// <returns>
		///		A value that indicates the relative order of the <see cref="ErrorInt32"/> and <see cref="object"/> being compared. The return value has these meanings:
		///		<list type="table">
		///			<listheader>
		///				<term>Value</term>
		///				<term>Meaning</term>
		///			</listheader>
		///			<item>
		///				<description>Less than zero</description>
		///				<description>This <see cref="ErrorInt32"/> precedes <paramref name="obj" /> in the sort order.</description>
		///			</item>
		///			<item>
		///				<description>Zero</description>
		///				<description>This <see cref="ErrorInt32"/> occurs in the same position in the sort order as <paramref name="obj" />.</description>
		///			</item>
		///			<item>
		///				<description>Greater than zero</description>
		///				<description>This <see cref="ErrorInt32"/> follows <paramref name="obj" /> in the sort order.</description>
		///			</item>
		///		</list>
		/// </returns>
		/// <exception cref="ArgumentException"><paramref name="obj"/> is not an <see cref="ErrorInt32"/> or an <see cref="int"/>.</exception>
		public int CompareTo(object obj)
		{
			if (obj is ErrorInt32)
			{
				return CompareTo((ErrorInt32)obj);
			}
			if (obj is int)
			{
				return CompareTo((int)obj);
			}
			if (ReferenceEquals(obj, null))
			{
				return 1;
			}
			throw Exceptions.ParameterIsNotTheCorrectType<ErrorInt32>(nameof(obj));
		}

		/// <summary>
		///		Indicates whether the current <see cref="ErrorInt32"/> is equal to another <see cref="ErrorInt32"/>.
		/// </summary>
		/// <param name="other">An <see cref="ErrorInt32"/> to compare with this <see cref="ErrorInt32"/>.</param>
		/// <returns>
		///		<see langword="true"/> if the current <see cref="ErrorInt32"/> is equal to the <paramref name="other" /> parameter; otherwise, <see langword="false"/>.
		/// </returns>
		public bool Equals(ErrorInt32 other) => value.Equals(other.value);

		/// <summary>
		///		Indicates whether the current <see cref="ErrorInt32"/> is equal to an <see cref="int"/>.
		/// </summary>
		/// <param name="other">An <see cref="int"/> to compare with this <see cref="ErrorInt32"/>.</param>
		/// <returns>
		///		<see langword="true"/> if the current <see cref="ErrorInt32"/> is equal to the <paramref name="other" /> parameter; otherwise, <see langword="false"/>.
		/// </returns>
		public bool Equals(int other) => value.Equals(other);

		/// <summary>
		///		Indicates whether the current <see cref="ErrorInt32"/> is equal to an <see cref="object"/>.
		/// </summary>
		/// <param name="obj">An <see cref="object"/> to compare with this <see cref="ErrorInt32"/>.</param>
		/// <returns>
		///		<see langword="true"/> if <paramref name="obj"/> is an <see cref="ErrorInt32"/> or an <see cref="int"/> and the current <see cref="ErrorInt32"/> is equal to the <paramref name="obj" /> parameter; otherwise, <see langword="false"/>.
		/// </returns>
		public override bool Equals(object obj)
		{
			if (obj is ErrorInt32)
			{
				return Equals((ErrorInt32)obj);
			}
			if (obj is int)
			{
				return Equals((int)obj);
			}
			return false;
		}

		/// <summary>
		///		Returns a hash code for this <see cref="ErrorInt32"/>.
		/// </summary>
		/// <returns>
		///		A hash code for this <see cref="ErrorInt32"/>, suitable for use in hashing algorithms and data structures like a hash table.
		/// </returns>
		public override int GetHashCode() => value.GetHashCode();

		/// <summary>
		///		Returns a <see cref="string" /> that represents this <see cref="ErrorInt32"/>.
		/// </summary>
		/// <returns>
		///		A <see cref="string" /> that represents this <see cref="ErrorInt32"/>.
		/// </returns>
		public override string ToString() => value.ToString();

		/// <summary>
		///		Returns a <see cref="string" /> that represents this <see cref="ErrorInt32"/>.
		/// </summary>
		/// <param name="format">The format.</param>
		/// <param name="formatProvider">The format provider.</param>
		/// <returns>
		///		A <see cref="string" /> that represents this <see cref="ErrorInt32"/>.
		/// </returns>
		public string ToString(string format, IFormatProvider formatProvider) => value.ToString(format, formatProvider);

		public static implicit operator int(ErrorInt32 @this) => @this.value;

		public static implicit operator ErrorInt32(int @this) => new ErrorInt32(@this);

		public static implicit operator ImageflowError(ErrorInt32 @this) => (ImageflowError)@this.value;

		public static explicit operator ErrorInt32(ImageflowError @this) => new ErrorInt32((int)@this);
	}
}