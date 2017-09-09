using System;
using System.Collections.Generic;
using System.Linq;
using Imageflow.Bindings;

namespace Imageflow.Fluent
{
    public class BuildJobResult
    {
        //TODO: add dimensions, etc 
        
        private JsonResponse _response;
        private Dictionary<int, IOutputDestination> _outputs;
        
        public static BuildJobResult From(JsonResponse response, Dictionary<int, IOutputDestination> outputs)
        {
            return new BuildJobResult { _response =  response, _outputs =  outputs};
        }

        //TODO: improve these and add fallible methods
        public ArraySegment<byte> GetOutputBytes(int ioId) => ((BytesDestination) _outputs[ioId]).GetBytes();
        public ArraySegment<byte> GetFirstOutputBytes() => ((BytesDestination) _outputs.Values.First()).GetBytes();
       
    }
}
