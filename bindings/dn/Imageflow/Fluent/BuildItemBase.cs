using System.Threading;

namespace Imageflow.Fluent
{
    public class BuildItemBase
    {
        internal FluentBuildJob Builder { get; }
        internal BuildNode Input { get; }
        internal BuildNode Canvas { get; }
        internal object NodeData { get;  }
        internal long Uid { get;  }
        public override bool Equals(object obj) => Uid == (obj as BuildItemBase)?.Uid;
        public override int GetHashCode() => (int) Uid; //We probably don't need to worry about more than 2 billion instances? 
        
        private static long _next = 0;
        private static long NextUid() => Interlocked.Increment(ref _next);
        internal bool IsEmpty => NodeData == null;
        
        protected BuildItemBase(FluentBuildJob builder, object nodeData, BuildNode inputNode, BuildNode canvasNode)
        {
            Builder = builder;
            Input = inputNode;
            Canvas = canvasNode;
            NodeData = nodeData;
            Uid = NextUid();
            builder.AddNode(this);
        }
    }
}