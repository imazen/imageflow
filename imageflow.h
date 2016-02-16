#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>

#include "fastscaling.h"


//Version selection is not implemented within imageflow, instead, we let callers do that logic:
//Expose API to evaluate graph and suggest minimum source dimensions.
//Returns "indeterminate" if face or whitespace cropping is in use, or any other conditionals.

//Source/output files and I/O interfaces must be registered with the context. They can survive multiple ImageJobs.

//ImageJobs associate an opaque cache for dimensions, metadata, and (potentially) bitmap data with these I/O interfaces.



typedef enum flow_ntype {
    flow_ntype_Null = 0,
    flow_ntype_primitive_Flip_Vertical = 1,
    flow_ntype_primitive_Crop = 2, //Creates a new window into an existing frame -
    flow_ntype_primitive_CopyRectToCanvas = 3, //Overwrite only, no compositing
    flow_ntype_primitive_CreateCanvas = 4, //blank, or with background color
    flow_ntype_primitive_RenderToCanvas1D = 5,
    flow_ntype_primitive_Halving = 6,

    flow_ntype_primitive_bitmap_bgra_pointer,
    flow_ntype_primitive_Encode_PngFrame,
    flow_ntype_primitive_Encode_Jpeg,
    flow_ntype_primitive_Encode_Gif,
    flow_ntype_primitive_Decode_Png,
    flow_ntype_primitive_Decode_Jpeg,
    flow_ntype_primitive_Decode_Gif,
    flow_ntype_primitive_Metadata_Destination,


    flow_ntype_Create_Canvas = 256,
    flow_ntype_Crop_Percentage,
    flow_ntype_Crop_Percentage_Infinite_Canvas, //canvas_color
    flow_ntype_Crop_Rectangle,
    flow_ntype_Flip_Vertical,
    flow_ntype_Flip_Horizontal,
    flow_ntype_Rotate_90,
    flow_ntype_Rotate_180,
    flow_ntype_Rotate_270,
    flow_ntype_Rotate_Flip_Per_Orientation,
    flow_ntype_Scale, //(preserve colorspace), interpolation filter
    flow_ntype_Constrain, //(mode=pad|max|crop|stretch) (width, height) (scale=down|up|both|canvas) (anchor=9 points)
    flow_ntype_Matte,
    flow_ntype_EnlargeCanvas,
    flow_ntype_Sharpen,
    flow_ntype_Blur,
    flow_ntype_Convolve_Custom,
    flow_ntype_AdjustContrast,
    flow_ntype_AdjustSaturation,
    flow_ntype_AdjustBrightness,
    flow_ntype_CropWhitespace, //tolerances and padding
    flow_ntype_Opacity,
    flow_ntype_Sepia,
    flow_ntype_Grayscale, //true|y|ry|ntsc|bt709|flat
    flow_ntype_DrawImage,
    flow_ntype_RemoveNoise,
    flow_ntype_ColorMatrixsRGB,
    flow_ntype_Resource_Placeholder,
    flow_ntype__FORCE_ENUM_SIZE_INT32 = 2147483647
} flow_ntype;

typedef enum flow_edge_type {
    flow_edgetype_null,
    flow_edgetype_input,
    flow_edgetype_canvas,
    flow_edgetype_FORCE_ENUM_SIZE_INT32 = 2147483647
} flow_edge_type;

struct flow_edge {
    flow_edge_type type;
    int32_t from;
    int32_t to;
    int32_t info_byte_index;
    int32_t info_bytes;
};

struct flow_node {
    flow_ntype type;
    int32_t info_byte_index;
    int32_t info_bytes;
} ;


struct flow_graph {
    uint32_t memory_layout_version; //This progresses differently from the library version, as internals are subject to refactoring. If we are given a graph to copy, we check this number.
    struct flow_edge * edges;
    int32_t edge_count;
    int32_t next_edge_id;
    int32_t max_edges;

    struct flow_node * nodes;
    int32_t node_count;
    int32_t next_node_id;
    int32_t max_nodes;

    uint8_t * info_bytes;
    int32_t max_info_bytes;
    int32_t next_info_byte;
    int32_t deleted_bytes;

    float growth_factor;
};


struct flow_graph *flow_graph_create(Context *c, uint32_t max_edges, uint32_t max_nodes, uint32_t max_info_bytes, float growth_factor);

void flow_graph_destroy(Context *c, struct flow_graph *target);

bool flow_graph_replace_if_too_small(Context *c,  struct flow_graph ** g, uint32_t free_nodes_required, uint32_t free_edges_required, uint32_t free_bytes_required);
struct flow_graph *flow_graph_copy_and_resize(Context *c, struct flow_graph * from, uint32_t max_edges, uint32_t max_nodes, uint32_t max_info_bytes);

struct flow_graph *flow_graph_memcpy(Context *c, struct flow_graph * from);

int32_t flow_graph_copy_info_bytes_to(Context *c, struct flow_graph *from, struct flow_graph **to, int32_t byte_index,
                                      int32_t byte_count);

int32_t flow_edge_duplicate(Context *c, struct flow_graph **g, int32_t edge_id);
/*
 * flow_Graph
 * flow_Node
 * flow_Edge
 * flow_edge_type
 * flow_ntype
 *
 * flow_node_create_canvas
 * flow_
 */


int32_t flow_node_create_canvas(Context *c, struct flow_graph **g, int32_t prev_node, BitmapPixelFormat format,
                                size_t width, size_t height, uint32_t bgcolor);
int32_t flow_node_create_scale(Context *c, struct flow_graph **g, int32_t prev_node, size_t width, size_t height);


int32_t flow_node_create_resource_placeholder(Context *c, struct flow_graph **g, int32_t prev_node,
                                              int32_t output_slot_id);


int32_t flow_node_create_resource_bitmap_bgra(Context *c, struct flow_graph ** graph_ref, int32_t prev_node, BitmapBgra ** ref);



bool flow_node_delete(Context *c, struct flow_graph *g, int32_t node_id);

bool flow_edge_delete(Context *c, struct flow_graph *g, int32_t edge_id);

bool flow_edge_delete_all_connected_to_node(Context *c, struct flow_graph *g, int32_t node_id);

bool flow_graph_duplicate_edges_to_another_node(Context *c,  struct flow_graph ** g, int32_t from_node, int32_t to_node);

struct flow_nodeinfo_index {
    int32_t index;
};

struct flow_nodeinfo_createcanvas {
    BitmapPixelFormat format;
    size_t width;
    size_t height;
    uint32_t bgcolor;
};

struct flow_nodeinfo_size {
    size_t width;
    size_t height;
};


struct flow_nodeinfo_resource_bitmap_bgra {
    BitmapBgra ** ref;
};


struct flow_job;


typedef enum FLOW_DIRECTION{
    FLOW_OUTPUT = 8,
    FLOW_INPUT = 4
} FLOW_DIRECTION;

struct flow_job * flow_job_create(Context *c);
void flow_job_destroy(Context *c, struct flow_job * job);

struct flow_graph * flow_job_complete_graph(Context *c, struct flow_job * job, struct flow_graph * graph);

bool flow_job_execute_graph(Context *c, struct flow_job * job, struct flow_graph * graph);


struct flow_graph * flow_graph_flatten(Context *c, struct flow_graph * graph, bool free_previous_graph);

int32_t flow_job_add_bitmap_bgra(Context *c, struct flow_job * job, FLOW_DIRECTION dir, int32_t placeholder);



BitmapBgra * flow_job_get_bitmap_bgra(Context *c, struct flow_job * job, int32_t resource_id);

void flow_graph_print_to(Context *c, struct flow_graph *g, FILE * stream);


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


typedef enum flow_scanlines_filter_type {
    flow_scanlines_filter_Sharpen, //3x3, percentage-based
    flow_scanlines_filter_Blur, //3x box blur to simulate guassian
    flow_scanlines_filter_Convolve, //Apply convolution kernel
    flow_scanlines_filter_ColorMatrix, //Apply color matrix
    flow_scanlines_filter_ToLinear,
    flow_scanlines_filter_ToSrgb,
    flow_scanlines_filter_Custom, //Execute custom callback.,
    flow_scanlines_filter__FORCE_ENUM_SIZE_INT32 = 2147483647
} flow_scanlines_filter_type;

struct flow_scanlines_filter;

struct flow_scanlines_filter {
    flow_scanlines_filter_type type;
    struct flow_scanlines_filter *next;
};
struct flow_nodeinfo_render_to_canvas_1d {
    InterpolationDetails * interpolationDetails;
   // CompositionMode compose;
    bool transpose_on_write;
    //Floatspace working_space;
    struct flow_scanlines_filter filter_list;
};


//Pick frame
//decoding
//encoding
//quantization



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

