using System;
using System.IO;
using Newtonsoft.Json;

namespace Imageflow.Native.JSON
{
	internal class IoObject
	{
		public Stream Data
		{
			get;
		}

		public int Id
		{
			get;
		}

		public Direction Direction
		{
			get;
		}

		internal IoObject(JsonReader reader)
		{
			byte init = 0;
			while (reader.Read() && reader.TokenType != JsonToken.EndObject)
			{
				if (reader.TokenType != JsonToken.PropertyName)
				{
					throw new JsonException("Not at object start.");
				}
				string propertyName = reader.ReadAsString();
				if (!reader.Read())
				{
					throw Exceptions.NoValue(propertyName);
				}
				switch (propertyName)
				{
					case "io_id":
						if (reader.TokenType != JsonToken.Integer)
						{
							throw new ArgumentException("Not an integer", propertyName);
						}
						Id = reader.ReadAsInt32() ?? -1;
						init++;
						break;
					case "io":
						if (reader.TokenType != JsonToken.StartObject)
						{
							throw new ArgumentException();
						}
						if (!reader.Read())
						{
							throw new ArgumentException();
						}
						if (reader.TokenType != JsonToken.PropertyName)
						{
							throw new ArgumentException();
						}
						string ioType = reader.ReadAsString();
						if (!reader.Read())
						{
							throw new ArgumentException();
						}
						if (reader.TokenType != JsonToken.String)
						{
							throw new ArgumentException();
						}
						switch (ioType)
						{
							case "base_64":
								Data = new MemoryStream(Convert.FromBase64String(reader.ReadAsString()));
								break;
							case "file":
								Data = new FileStream(reader.ReadAsString(), FileMode.Open, FileAccess.Read, FileShare.Read, 4096, FileOptions.Asynchronous);
								break;
							default:
								throw new ArgumentException();
						}
						init++;
						break;
					case "direction":
						if (reader.TokenType != JsonToken.String)
						{
							throw new JsonException();
						}
						switch (reader.ReadAsString())
						{
							case "in":
								Direction = Direction.In;
								break;
							case "out":
								Direction = Direction.Out;
								break;
							default:
								throw new JsonException();
						}
						init++;
						break;
				}
			}
			if (init != 3)
			{
				throw new JsonException();
			}
		}
	}
}
