using System.Threading;

namespace Imageflow
{
	/// <summary>
	///		Very simple cross-thread preventions lock.
	/// </summary>
	internal struct LightLock
	{
		/// <summary>
		///		The true value.
		/// </summary>
		private const int TRUE = 1;
		/// <summary>
		///		The false value.
		/// </summary>
		private const int FALSE = 0;

		/// <summary>
		///		If the lock has been taken.
		/// </summary>
		private int taken;

		/// <summary>
		///		Takes the lock.
		/// </summary>
		/// <returns></returns>
		public bool Take() => Interlocked.CompareExchange(ref taken, TRUE, FALSE) == FALSE;

		/// <summary>
		///		Release the lock.
		/// </summary>
		public void Release() => Interlocked.Exchange(ref taken, FALSE);
	}
}
