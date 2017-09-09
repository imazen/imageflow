using System.Collections;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Threading;
using System.Threading.Tasks;
using Imageflow;
using Imageflow.Bindings;
using Microsoft.IO;

namespace Imageflow.Fluent
{

  
    public class BuildEndpoint : BuildItemBase
    {
        internal BuildEndpoint(FluentBuildJob builder,object nodeData, BuildNode inputNode, BuildNode canvasNode) : base(builder, nodeData, inputNode,
            canvasNode){}


        public Task<BuildJobResult> FinishAsync() => Builder.FinishAsync();

        public Task<BuildJobResult> FinishAsync(CancellationToken cancellationToken) =>
            Builder.FinishAsync(cancellationToken);
        
    }
}
