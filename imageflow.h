#pragma once

#ifdef __cplusplus
extern "C" {
#endif

//Version selection is not implemented within imageflow, instead, we let callers do that logic:
//Expose API to evaluate graph and suggest minimum source dimensions.
//Returns "indeterminate" if face or whitespace cropping is in use, or any other conditionals.

//Source images must be registered with the context. They can survive multiple ImageJobs.
//They contain an opaque cache for dimensions, metadata, and (potentially) bitmap data

//There must be a primary source image; only one image can be 'looped'.


//Multi-frame/multi-page images are not magically handled.
//We require one frame graph per frame/page to be created by the client after metadata is parsed for that frame/page.


/*
 * output format:
 *
 *
 * png -> png
 * png -> jpeg
 * jpeg -> png
 * gif -> png
 * agif -> agif
 *
 *
 *
 *
 */


//Imageflow makes multiple passes over each graph
// 1. Parse headers on every source node. Resolve conditionals based on that data.
// 2. ...

// n-1: Collapse frame graph into primitives

//Source node - can be asked for different I/O interfaces. May always upgrade instead. Header should be implemented for safety (avoiding expensive loads)
//header, length -> random access -> buffer

//Output node - callback
//Output node - metadata

//File output nodes
//Output node - buffer
//Output node - random read/write

//MetadataCache

// SelectFrame
// PerFrameFlow - contains subgraph, which has an FrameOutput endpoint.

typedef enum ImageNodeType {
    ImageNode_SelectFrame,
    ImageNode_PerFrameFlow,
    ImageNode_MetadataCache,

    ImageNode_EncodePng,
    ImageNode_EncodeJpeg,
    ImageNode_EncodeGif,
    ImageNode_DecodePng,
    ImageNode_DecodeJpeg,
    ImageNode_DecodeGif,
    ImageNode_FileSource,
    ImageNode_FileDestination,
    ImageDone_MetadataDestination,



};

typedef enum ScanlinesFilterType{
    Scanlines_Sharpen, //3x3, percentage-based
    Scanlines_Blur, //3x box blur to simulate guassian
    Scanlines_Convolve, //Apply convolution kernel
    Scanlines_ColorMatrix, //Apply color matrix
    Scanlines_ToLinear,
    Scanlines_ToSrgb,
    Scanlines_Custom //Execute custom callback.
};

struct ScanlinesFilter{
    ScanlinesFilterType type;
    ScanlinesFilter next;
};
struct FrameNode_RenderToCanvas1D{
    InterpolationDetails * interpolationDetails;
   // CompositionMode compose;
    bool transpose_on_write;
    //Floatspace working_space;
    ScanlinesFilter filter_list;
};
typedef enum FrameNodeType{
    Primitive_Flip_Vertical = 1,
    Primitive_Crop = 2, //Creates a new window into an existing frame -
    Primitive_CopyRectToCanvas = 3, //Overwrite only, no compositing
    Primitive_CreateCanvas = 4, //blank, or with background color
    Primitive_RenderToCanvas1D = 5,
    Primitive_Halving = 6,

    Filter_Crop_Percentage = 200,
    Filter_Crop_Percentage_Infinite_Canvas, //canvas_color
    Filter_Crop_Rectangle = 201,
    Filter_Flip_Vertical = 300,
    Filter_Flip_Horizontal = 301,
    Filter_Rotate_90 = 131,
    Filter_Rotate_180 = 132,
    Filter_Rotate_270 = 133,
    Filter_Rotate_Flip_Per_Orientation = 134,
    Filter_Scale, //(preserve colorspace), interpolation filter
    Filter_Constrain, //(mode=pad|max|crop|stretch) (width, height) (scale=down|up|both|canvas) (anchor=9 points)
    Filter_Matte,
    Filter_EnlargeCanvas,
    Filter_Sharpen,
    Filter_Blur,
    Filter_Convolve_Custom,
    Filter_AdjustContrast,
    Filter_AdjustSaturation,
    Filter_AdjustBrightness,
    Filter_CropWhitespace, //tolerances and padding
    Filter_Opacity,
    Filter_Sepia,
    Filter_Grayscale, //true|y|ry|ntsc|bt709|flat
    Filter_DrawImage,
    Filter_RemoveNoise,
    Filter_ColorMatrixsRGB,
    Filter_Input_Placeholder,
    Filter_Output_Placeholder

};

//Pick frame
//decoding
//encoding
//quantization



struct {
    FrameNodeType type;
    void * data;


} FrameNode;

//
//| VFlip | Format agnostic | In Place
//| Crop  | Format agnostic | In Place
//| CopyRect  | Format agnostic | New Frame
//| CreateCanvas |
//| RenderToCanvas1D (scale (InterpolationDetails), compose (InPlace, Copy, Blende, Matte[color]), bool transpose, [list of convolution & pixel filters], working_floatspace)
//
//


#ifdef __cplusplus
}
#endif

