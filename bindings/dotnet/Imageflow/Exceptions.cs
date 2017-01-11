#pragma warning disable HeapAnalyzerExplicitNewObjectRule // Explicit new reference type allocation
using System;
using Newtonsoft.Json;

namespace Imageflow
{
	/// <summary>
	///		Common Exceptions
	/// </summary>
	internal static class Exceptions
	{
		/// <summary>
		///		Parameter is not the correct.
		/// </summary>
		/// <typeparam name="TWantedType">The type of the wanted type.</typeparam>
		/// <param name="nameOfParameter">The name of parameter.</param>
		/// <returns>An <see cref="ArgumentException"/> with filled in message and parameter.</returns>
		public static ArgumentException ParameterIsNotTheCorrectType<TWantedType>(string nameOfParameter) => new ArgumentException($"{nameOfParameter} is not an {typeof(TWantedType).Name}.");

		/// <summary>
		///		No value for JSON property.
		/// </summary>
		/// <param name="propertyName">Name of the property.</param>
		/// <returns>A <see cref="JsonException"/> with filled message.</returns>
		public static JsonException NoValue(string propertyName) => new JsonException($"No value for property {propertyName}.");
	}
}
#pragma warning restore HeapAnalyzerExplicitNewObjectRule // Explicit new reference type allocation
