using System;
using System.Drawing;
using System.IO;

namespace Imageflow.Fluent
{
    public class BuildNode :BuildItemBase
    {
        internal static BuildNode StartNode(FluentBuildJob graph, object data) => new BuildNode(graph, data, null, null);
    
        
        public BuildEndpoint Encode(IOutputDestination destination, int ioId, IEncoderPreset encoderPreset)
        {
            Builder.AddOutput(ioId, destination);
            return new BuildEndpoint(Builder,
                new {encode = new {io_id = ioId, preset = encoderPreset?.ToImageflowDynamic()}}, this, null);
        }

        public BuildEndpoint Encode(IOutputDestination destination, IEncoderPreset encoderPreset) =>
            Encode( destination, Builder.GenerateIoId(), encoderPreset);
        

        public BuildEndpoint EncodeToBytes(int ioId, IEncoderPreset encoderPreset) =>
            Encode(new BytesDestination(), ioId, encoderPreset);
        public BuildEndpoint EncodeToBytes(IEncoderPreset encoderPreset) =>
            Encode(new BytesDestination(), encoderPreset);
        
        public BuildEndpoint EncodeToStream(Stream stream, bool disposeStream, int ioId, IEncoderPreset encoderPreset) =>
            Encode(new StreamDestination(stream, disposeStream), ioId, encoderPreset);
        public BuildEndpoint EncodeToStream(Stream stream, bool disposeStream, IEncoderPreset encoderPreset) =>
            Encode(new StreamDestination(stream, disposeStream), encoderPreset);
        
        
        private BuildNode(FluentBuildJob builder,object nodeData, BuildNode inputNode, BuildNode canvasNode) : base(builder, nodeData, inputNode,
            canvasNode){}

        private BuildNode To(object data) => new BuildNode(Builder, data, this, null);
        private BuildNode NodeWithCanvas(BuildNode canvas, object data) => new BuildNode(Builder, data, this, canvas);


        public BuildNode ConstrainWithin(uint? w, uint? h) => To(new {constrain = new {w, h}});
        public BuildNode ConstrainWithin(uint? w, uint? h, float? sharpenPercent, InterpolationFilter? downFilter, InterpolationFilter? upFilter, ScalingFloatspace? interpolationColorspace, ResampleWhen? resampleWhen)
            => To(new {constrain = new {w, h, hints = new
            {
                sharpen_percent = sharpenPercent,
                down_filter = downFilter?.ToString().ToLowerInvariant(),
                up_filter = upFilter?.ToString().ToLowerInvariant(),
                scaling_colorspace = interpolationColorspace?.ToString().ToLowerInvariant(),
                resample_when = resampleWhen?.ToString().ToLowerInvariant()
            }}});


     
        public BuildNode FlipVertical() => To(new {flip_v = (string)null});
        public BuildNode FlipHorizontal() => To(new {flip_h = (string)null });
        
        public BuildNode Rotate90() => To(new {rotate_90 = (string)null });
        public BuildNode Rotate180() => To(new {rotate_180 = (string)null });
        public BuildNode Rotate270() => To(new {rotate_270 = (string)null });
        public BuildNode Transpose() => To(new {transpose = (string)null });

        public BuildNode Branch(Func<BuildNode, BuildEndpoint> f)
        {
            f(this);
            return this;
        } 

        public BuildNode CopyRectTo(BuildNode canvas, Rectangle area, Point to) => NodeWithCanvas(canvas, new
        {
            copy_rect_to_canvas = new
            {
                from_x = area.X,
                from_y = area.Y,
                width = area.Width,
                height = area.Height,
                x = to.X,
                y = to.Y
            }
        });

//        public BuildNode Clone() => new BuildNode(NodeData,Input,Canvas,Uid);
//        public BuildNode Branch() => Clone();
        
//        public FluentGraphBuilder Builder() => new FluentGraphBuilder(this);
//        public object ToBuildMessage() => Builder().to_framewise().
    }
}
