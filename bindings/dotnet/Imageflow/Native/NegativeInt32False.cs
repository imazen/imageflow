using System;
using System.Runtime.InteropServices;

namespace Imageflow.Native
{
	/// <summary>
	///		Return from a native method that returns negative values for error.
	/// </summary>
	/// <seealso cref="IComparable" />
	/// <seealso cref="IComparable{T}" />
	/// <seealso cref="IEquatable{T}" />
	/// <seealso cref="IFormattable" />
	[StructLayout(LayoutKind.Sequential)]
	internal struct NegativeInt32False : IComparable, IComparable<int>, IEquatable<int>, IFormattable, IComparable<NegativeInt32False>, IEquatable<NegativeInt32False>
	{
		/// <summary>
		///		The value
		/// </summary>
		private readonly int value;

		/// <summary>
		///		Initializes a new instance of the <see cref="NegativeInt32False"/> struct.
		/// </summary>
		/// <param name="value">The value.</param>
		public NegativeInt32False(int value)
		{
			this.value = value;
		}

		/// <summary>
		///		Compares the current <see cref="NegativeInt32False"/> with another <see cref="NegativeInt32False"/> and returns an integer that indicates whether the current <see cref="NegativeInt32False"/> precedes, follows, or occurs in the same position in the sort order as the other <see cref="NegativeInt32False"/>.
		/// </summary>
		/// <param name="other">An <see cref="NegativeInt32False"/> to compare with this <see cref="NegativeInt32False"/>.</param>
		/// <returns>
		///		A value that indicates the relative order of the <see cref="NegativeInt32False"/>s being compared. The return value has these meanings:
		///		<list type="table">
		///			<listheader>
		///				<term>Value</term>
		///				<term>Meaning</term>
		///			</listheader>
		///			<item>
		///				<description>Less than zero</description>
		///				<description>This <see cref="NegativeInt32False"/> precedes <paramref name="other" /> in the sort order.</description>
		///			</item>
		///			<item>
		///				<description>Zero</description>
		///				<description>This <see cref="NegativeInt32False"/> occurs in the same position in the sort order as <paramref name="other" />.</description>
		///			</item>
		///			<item>
		///				<description>Greater than zero</description>
		///				<description>This <see cref="NegativeInt32False"/> follows <paramref name="other" /> in the sort order.</description>
		///			</item>
		///		</list>
		/// </returns>
		public int CompareTo(NegativeInt32False other) => value.CompareTo(other.value);

		/// <summary>
		///		Compares the current <see cref="NegativeInt32False"/> with an <see cref="int"/> and returns an integer that indicates whether the current <see cref="NegativeInt32False"/> precedes, follows, or occurs in the same position in the sort order as the <see cref="int"/>.
		/// </summary>
		/// <param name="other">An <see cref="int"/> to compare with this <see cref="NegativeInt32False"/>.</param>
		/// <returns>
		///		A value that indicates the relative order of the <see cref="NegativeInt32False"/> and <see cref="int"/> being compared. The return value has these meanings:
		///		<list type="table">
		///			<listheader>
		///				<term>Value</term>
		///				<term>Meaning</term>
		///			</listheader>
		///			<item>
		///				<description>Less than zero</description>
		///				<description>This <see cref="NegativeInt32False"/> precedes <paramref name="other" /> in the sort order.</description>
		///			</item>
		///			<item>
		///				<description>Zero</description>
		///				<description>This <see cref="NegativeInt32False"/> occurs in the same position in the sort order as <paramref name="other" />.</description>
		///			</item>
		///			<item>
		///				<description>Greater than zero</description>
		///				<description>This <see cref="NegativeInt32False"/> follows <paramref name="other" /> in the sort order.</description>
		///			</item>
		///		</list>
		/// </returns>
		public int CompareTo(int other) => value.CompareTo(other);

		/// <summary>
		///		Compares the current <see cref="NegativeInt32False"/> with an <see cref="object"/> and returns an integer that indicates whether the current <see cref="NegativeInt32False"/> precedes, follows, or occurs in the same position in the sort order as the <see cref="object"/>.
		/// </summary>
		/// <param name="obj">An <see cref="object"/> to compare with this <see cref="NegativeInt32False"/>.</param>
		/// <returns>
		///		A value that indicates the relative order of the <see cref="NegativeInt32False"/> and <see cref="object"/> being compared. The return value has these meanings:
		///		<list type="table">
		///			<listheader>
		///				<term>Value</term>
		///				<term>Meaning</term>
		///			</listheader>
		///			<item>
		///				<description>Less than zero</description>
		///				<description>This <see cref="NegativeInt32False"/> precedes <paramref name="obj" /> in the sort order.</description>
		///			</item>
		///			<item>
		///				<description>Zero</description>
		///				<description>This <see cref="NegativeInt32False"/> occurs in the same position in the sort order as <paramref name="obj" />.</description>
		///			</item>
		///			<item>
		///				<description>Greater than zero</description>
		///				<description>This <see cref="NegativeInt32False"/> follows <paramref name="obj" /> in the sort order.</description>
		///			</item>
		///		</list>
		/// </returns>
		/// <exception cref="ArgumentException"><paramref name="obj"/> is not an <see cref="NegativeInt32False"/> or an <see cref="int"/>.</exception>
		public int CompareTo(object obj)
		{
			if (obj is NegativeInt32False)
			{
				return CompareTo((NegativeInt32False)obj);
			}
			if (obj is int)
			{
				return CompareTo((int)obj);
			}
			if (ReferenceEquals(obj, null))
			{
				return 1;
			}
			throw Exceptions.ParameterIsNotTheCorrectType<NegativeInt32False>(nameof(obj));
		}

		/// <summary>
		///		Indicates whether the current <see cref="NegativeInt32False"/> is equal to another <see cref="NegativeInt32False"/>.
		/// </summary>
		/// <param name="other">An <see cref="NegativeInt32False"/> to compare with this <see cref="NegativeInt32False"/>.</param>
		/// <returns>
		///		<see langword="true"/> if the current <see cref="NegativeInt32False"/> is equal to the <paramref name="other" /> parameter; otherwise, <see langword="false"/>.
		/// </returns>
		public bool Equals(NegativeInt32False other) => value.Equals(other.value);

		/// <summary>
		///		Indicates whether the current <see cref="NegativeInt32False"/> is equal to an <see cref="int"/>.
		/// </summary>
		/// <param name="other">An <see cref="int"/> to compare with this <see cref="NegativeInt32False"/>.</param>
		/// <returns>
		///		<see langword="true"/> if the current <see cref="NegativeInt32False"/> is equal to the <paramref name="other" /> parameter; otherwise, <see langword="false"/>.
		/// </returns>
		public bool Equals(int other) => value.Equals(other);

		/// <summary>
		///		Indicates whether the current <see cref="NegativeInt32False"/> is equal to an <see cref="object"/>.
		/// </summary>
		/// <param name="obj">An <see cref="object"/> to compare with this <see cref="NegativeInt32False"/>.</param>
		/// <returns>
		///		<see langword="true"/> if <paramref name="obj"/> is an <see cref="NegativeInt32False"/> or an <see cref="int"/> and the current <see cref="NegativeInt32False"/> is equal to the <paramref name="obj" /> parameter; otherwise, <see langword="false"/>.
		/// </returns>
		public override bool Equals(object obj)
		{
			if (obj is NegativeInt32False)
			{
				return Equals((NegativeInt32False)obj);
			}
			if (obj is int)
			{
				return Equals((int)obj);
			}
			return false;
		}

		/// <summary>
		///		Returns a hash code for this <see cref="NegativeInt32False"/>.
		/// </summary>
		/// <returns>
		///		A hash code for this <see cref="NegativeInt32False"/>, suitable for use in hashing algorithms and data structures like a hash table.
		/// </returns>
		public override int GetHashCode() => value.GetHashCode();

		/// <summary>
		///		Returns a <see cref="string" /> that represents this <see cref="NegativeInt32False"/>.
		/// </summary>
		/// <returns>
		///		A <see cref="string" /> that represents this <see cref="NegativeInt32False"/>.
		/// </returns>
		public override string ToString() => value.ToString();

		/// <summary>
		///		Returns a <see cref="string" /> that represents this <see cref="NegativeInt32False"/>.
		/// </summary>
		/// <param name="format">The format.</param>
		/// <param name="formatProvider">The format provider.</param>
		/// <returns>
		///		A <see cref="string" /> that represents this <see cref="NegativeInt32False"/>.
		/// </returns>
		public string ToString(string format, IFormatProvider formatProvider) => value.ToString(format, formatProvider);

		public static implicit operator int(NegativeInt32False @this) => @this.value;

		public static implicit operator NegativeInt32False(int @this) => new NegativeInt32False(@this);

		public static bool operator false(NegativeInt32False @this) => @this.value < 0;
		public static bool operator true(NegativeInt32False @this) => @this.value >= 0;

		public static implicit operator bool(NegativeInt32False @this) => @this.value >= 0;
	}
}