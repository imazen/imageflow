#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <glenn/png/png.h>
#include "fastscaling.h"


//Version selection is not implemented within imageflow, instead, we let callers do that logic:
//Expose API to evaluate graph and suggest minimum source dimensions.
//Returns "indeterminate" if face or whitespace cropping is in use, or any other conditionals.

//Source/output files and I/O interfaces must be registered with the context. They can survive multiple ImageJobs.

//ImageJobs associate an opaque cache for dimensions, metadata, and (potentially) bitmap data with these I/O interfaces.



typedef enum flow_ntype {
    flow_ntype_Null = 0,
    flow_ntype_primitive_Flip_Vertical = 1,
    flow_ntype_primitive_Flip_Horizontal = 1,
    flow_ntype_primitive_Crop = 2,
    flow_ntype_primitive_CopyRectToCanvas = 3, //Overwrite only, no compositing
    flow_ntype_Create_Canvas = 4,
    flow_ntype_primitive_RenderToCanvas1D = 5,

    flow_ntype_primitive_bitmap_bgra_pointer,
    flow_ntype_primitive_decoder,
    flow_ntype_primitive_encoder,

    flow_ntype_non_primitive_nodes_begin = 256,
    flow_ntype_Clone,
    flow_ntype_Transpose,

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


typedef enum flow_compositing_mode{
    flow_compositing_mode_overwrite,
    flow_compositing_mode_compose,
    flow_compositing_mode_blend_with_matte
} flow_compositing_mode;


struct flow_job;


typedef enum flow_job_resource_type{
    flow_job_resource_type_bitmap_bgra = 1,
    flow_job_resource_type_buffer = 2

} flow_job_resource_type;

typedef enum flow_job_codec_type{
    flow_job_codec_type_null,
    flow_job_codec_type_bitmap_bgra_pointer,
    flow_job_codec_type_decode_png,
    flow_job_codec_type_encode_png
} flow_job_codec_type;


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

struct flow_edge {
    flow_edge_type type;
    int32_t from;
    int32_t to;
    int32_t from_width;
    int32_t from_height;
    BitmapPixelFormat from_format;
    bool from_alpha_meaningful;
    int32_t info_byte_index;
    int32_t info_bytes;
};

struct flow_node {
    flow_ntype type;
    int32_t info_byte_index;
    int32_t info_bytes;
    bool executed;
    BitmapBgra * result_bitmap;
    uint32_t ticks_elapsed;
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

int32_t flow_node_create_primitive_flip_vertical(Context *c, struct flow_graph **g, int32_t prev_node);
int32_t flow_node_create_primitive_flip_horizontal(Context *c, struct flow_graph **g, int32_t prev_node);
int32_t flow_node_create_clone(Context *c, struct flow_graph **g, int32_t prev_node);

int32_t flow_node_create_transpose(Context *c, struct flow_graph **g, int32_t prev_node);

int32_t flow_node_create_rotate_90(Context *c, struct flow_graph **g, int32_t prev_node);

int32_t flow_node_create_rotate_180(Context *c, struct flow_graph **g, int32_t prev_node);

int32_t flow_node_create_rotate_270(Context *c, struct flow_graph **g, int32_t prev_node);

int32_t flow_node_create_resource_placeholder(Context *c, struct flow_graph **g, int32_t prev_node,
                                              int32_t output_slot_id);




int32_t flow_node_create_resource_bitmap_bgra(Context *c, struct flow_graph ** graph_ref, int32_t prev_node, BitmapBgra ** ref);

int32_t flow_node_create_primitive_copy_rect_to_canvas(Context *c, struct flow_graph **g, int32_t prev_node, uint32_t from_x, uint32_t from_y, uint32_t width, uint32_t height, uint32_t x, uint32_t y);

int32_t flow_node_create_primitive_crop(Context *c, struct flow_graph **g, int32_t prev_node, uint32_t x1, uint32_t x2, uint32_t y1, uint32_t y2);

int32_t flow_node_create_render_to_canvas_1d(Context *c, struct flow_graph **g, int32_t prev_node,
                                             bool transpose_on_write,
                                             uint32_t canvas_x,
                                             uint32_t canvas_y,
                                             int32_t scale_to_width,
                                             WorkingFloatspace scale_and_filter_in_colorspace,
                                             float sharpen_percent,
                                             flow_compositing_mode compositing_mode,
                                             uint8_t *matte_color[4],
                                             struct flow_scanlines_filter * filter_list,
                                             InterpolationFilter interpolation_filter);



bool flow_node_delete(Context *c, struct flow_graph *g, int32_t node_id);

bool flow_edge_delete(Context *c, struct flow_graph *g, int32_t edge_id);

bool flow_edge_delete_all_connected_to_node(Context *c, struct flow_graph *g, int32_t node_id);

int32_t flow_graph_get_inbound_edge_count_of_type(Context *c, struct flow_graph *g, int32_t node_id,
                                                  flow_edge_type type);
int32_t flow_graph_get_first_inbound_edge_of_type(Context *c, struct flow_graph *g, int32_t node_id,
                                                  flow_edge_type type);

bool flow_edge_has_dimensions(Context *c, struct flow_graph *g, int32_t edge_id);
bool flow_node_input_edges_have_dimensions(Context *c, struct flow_graph *g, int32_t node_id);
bool flow_graph_duplicate_edges_to_another_node(Context *c, struct flow_graph **g, int32_t from_node, int32_t to_node,
                                                bool copy_inbound, bool copy_outbound);

int32_t flow_edge_create(Context *c, struct flow_graph **g, int32_t from, int32_t to, flow_edge_type type);

typedef bool (*flow_graph_visitor)(Context *c, struct flow_job * job, struct flow_graph **graph_ref, int32_t id, bool * quit, bool * skip_outbound_paths, void * custom_data);

bool flow_graph_walk(Context *c, struct flow_job * job, struct flow_graph **graph_ref, flow_graph_visitor node_visitor,  flow_graph_visitor edge_visitor, void * custom_data );



struct flow_nodeinfo_index {
    int32_t index;
};

struct flow_nodeinfo_createcanvas {
    BitmapPixelFormat format;
    size_t width;
    size_t height;
    uint32_t bgcolor;
};

struct flow_nodeinfo_crop {
    uint32_t x1;
    uint32_t x2;
    uint32_t y1;
    uint32_t y2;
};

struct flow_nodeinfo_copy_rect_to_canvas {
    uint32_t x;
    uint32_t y;
    uint32_t from_x;
    uint32_t from_y;
    uint32_t width;
    uint32_t height;

};

struct flow_nodeinfo_size {
    size_t width;
    size_t height;
};


struct flow_nodeinfo_resource_bitmap_bgra {
    BitmapBgra ** ref;
};

struct flow_nodeinfo_codec {
    void * codec_state;
    flow_job_codec_type type;
};


struct flow_nodeinfo_render_to_canvas_1d{
    //There will need to be consistency checks against the createcanvas node

    InterpolationFilter interpolation_filter;
    //InterpolationDetails * interpolationDetails;
    int32_t scale_to_width;
    uint32_t canvas_x;
    uint32_t canvas_y;
    bool transpose_on_write;
    WorkingFloatspace scale_in_colorspace;

    float sharpen_percent_goal;

    flow_compositing_mode compositing_mode;
    //When using compositing mode blend_with_matte, this color will be used. We should probably define this as always being sRGBA, 4 bytes.
    uint8_t matte_color[4];

    struct flow_scanlines_filter * filter_list;
};

bool flow_node_execute_render_to_canvas_1d(Context *c, struct flow_job * job, BitmapBgra * input, BitmapBgra * canvas, struct flow_nodeinfo_render_to_canvas_1d * info);


typedef enum FLOW_DIRECTION{
    FLOW_OUTPUT = 8,
    FLOW_INPUT = 4
} FLOW_DIRECTION;

struct flow_job * flow_job_create(Context *c);
void flow_job_destroy(Context *c, struct flow_job * job);
bool flow_job_configure_recording(Context * c, struct flow_job * job, bool record_graph_versions, bool record_frame_images, bool render_last_graph, bool render_graph_versions, bool render_animated_graph);
    bool flow_job_insert_resources_into_graph(Context *c, struct flow_job *job, struct flow_graph **graph);

bool flow_job_populate_dimensions_where_certain(Context *c, struct flow_job * job, struct flow_graph **graph_ref);
//For doing execution cost estimates, we force estimate, then flatten, then calculate cost
bool flow_job_force_populate_dimensions(Context *c, struct flow_job * job, struct flow_graph **graph_ref);
bool flow_job_execute_where_certain(Context *c, struct flow_job *job, struct flow_graph **graph_ref);
bool flow_job_graph_fully_executed(Context *c, struct flow_job *job, struct flow_graph *g);

bool flow_job_notify_graph_changed(Context *c, struct flow_job *job, struct flow_graph * g);
bool flow_job_execute(Context *c, struct flow_job * job,struct flow_graph **graph_ref);

bool flow_graph_flatten_where_certain(Context *c, struct flow_graph ** graph_ref);

int32_t flow_job_add_bitmap_bgra(Context *c, struct flow_job * job, FLOW_DIRECTION dir, int32_t graph_placeholder_id, BitmapBgra * bitmap);

int32_t flow_job_add_buffer(Context *c, struct flow_job * job, FLOW_DIRECTION dir, int32_t graph_placeholder_id, void * buffer, size_t buffer_size, bool owned_by_job);


int32_t flow_node_create_generic(Context *c, struct flow_graph ** graph_ref, int32_t prev_node, flow_ntype type);

bool flow_graph_print_to_dot(Context *c, struct flow_graph *g, FILE * stream, const char * image_node_filename_prefix);


BitmapBgra * flow_job_get_bitmap_bgra(Context *c, struct flow_job * job, int32_t resource_id);
struct flow_job_resource_buffer * flow_job_get_buffer(Context *c, struct flow_job * job, int32_t resource_id);

void flow_graph_print_to(Context *c, struct flow_graph *g, FILE * stream);


struct flow_job_resource_buffer{
    void * buffer;
    size_t buffer_size;
    bool owned_by_job;
    void * codec_state;
};

struct flow_job_png_encoder_state {
    char *buffer;
    size_t size;
    struct flow_job_resource_buffer * output_resource;
};


bool png_write_frame(Context * c, struct flow_job * job, void * codec_state, BitmapBgra * frame);


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

