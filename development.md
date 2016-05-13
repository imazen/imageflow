

# Auto-formatting source code

On Ubuntu 14.04

1. Ensure you have clang-format installed and you can run it as `clang-format`

sudo apt-get install clang-format-3.5

sudo ln -s /usr/bin/clang-format-3.5 /usr/bin/clang-format


2. Install git-clang-format

sudo wget -O /usr/local/bin/git-clang-format https://raw.githubusercontent.com/llvm-mirror/clang/master/tools/clang-format/git-clang-format

sudo chmod +x /usr/local/bin/git-clang-format


3. Clean up that nasty commit you just pushed

git clang-format --commit HEAD~1

git commit -m"Reformatting"

4. Reformat the whole repository

clang-format -i {lib,tests,.}/*.{c,h,cpp,hpp}

5. Import code style .clion.codestyle.xml into CLion to reduce the number of differences clang-format creates.

## Using multiple versions of GCC

1. export CC=gcc-4.8
2. export CPP=g++-4.8
3. Edit `compiler.version` in ~/.conan/conan.conf 

Per https://github.com/conan-io/conan/issues/178
Destroy the ./build directory before trying again.(`rm -rf build`)


## Generating animated gifs of graph progression.

1. Switch to the directory holding the generated graph_version_XXX.dot files.
2. Ensure you have graphviz, gifsicle and avtools:  sudo apt-get install libav-tools graphviz gifsicle
3. Convert them to .png: `find . -type f -name '*.dot' -execdir dot -Tpng -Gsize=5,9\! -Gdpi=100  -O {} \;`
4. Assemble .gif: `avconv -i job_2_graph_version_%d.dot.png -pix_fmt rgb24 -y output.gif`
5: Add delay to frames, optimize: `gifsicle -d 200 output.gif -l=2 -O -o optimized.gif`



## Benchmarking to-do:

Vs. https://github.com/h2non/imaginary
Vs. libvips directly
Vs. Imagemagick


## Misc. resources

https://github.com/mm2/Little-CMS/blob/master/utils/jpgicc/iccjpeg.c


## Look at vectorization

    gcc -DFLOW_GCC_IDCT -fopt-info-vec-missed  -std=gnu11 -iquotelib  -ffast-math -funroll-loops -mfpmath=both -mtune=native -march=native -O3 lib/codecs_jpeg_idct_fast.c



## (out of date) API sketches

```

//TODO: Adapt these function signatures to deal with error reporting (or are we expecting the host language to panic/throw exception?)
//TODO: Add dispose hooks?

//ImageSourceBufferReader
size_t get_length(void * token, flow_context * c){
    //Get size of image from storage based on token.
}
size_t copy_to(uint8_t * buffer, size_t buffer_size, void * token, flow_context * c){
    //Copy image bytes to destination buffer, returning actual number of bytes copied (in case get_length overestimated)
    //May be called with a smaller buffer if only the header is required. May be called multiple times; caching is suggested.
}


//ImageSourceSequentialReader
size_t get_length(void * token, flow_context * c){
    //Get size of image from storage based on token.
}
size_t read_bytes(uint8_t * buffer, size_t buffer_size, void * token, flow_context * c){
    //Copy next set of image bytes to destination buffer, returning actual number of bytes copied (in case get_length overestimated)
    //May be called many times.
}

//ImageSourceIO
size_t custom_read(void *buffer, size_t size, void * token) {
    return fread(buffer, 1, size, (FILE *)token);
}
size_t custom_write(void *buffer, size_t size, void * token) {
    return (size_t)fwrite(buffer, 1, size, (FILE *)token);
}
int custom_seek(long offset, int origin, void * token) {
    return fseek((FILE *)handle, offset, origin);
}
long int custom_position(void * token) {
    return ftell((FILE *)token);
}
size_t custom_length(void * token){

}

//ImageSourcePeek
size_t peek_bytes(void *buffer, size_t requested_byte_count, int32_t * more_bytes_exist, void * token, flow_context * c){
//Returns actual byte count, which may be less than requested, either because fewer header bytes were cached by the host,
//or because the file is shorter. Check the more_bytes_exist flag  (0 - all file bytes sent, 1 - partial file sent)
}

// ImageSourceWriter
int write_bytes(void *buffer, size_t size, void * token, flow_context * c){
}


// initialize your own IO functions
ImageSourceIO io;
io.read_proc = custom_read;
io.write_proc = custom_write;
io.seek_proc = custom_seek;
io.tell_proc = custom_position;
io.length_proc = custom_length;

uint8_t * image_a_buffer = malloc(200);
size_t image_a_bytes = 200;

char * image_b_uuid = "124-515215-15251";

ImageSourceBufferReader image_b_buffer;
image_b_buffer.get_length = get_length;
image_b_buffer.copy_to = copy_to;

//Source complicates
// Color profile is orthogonal to orientation data
//

flow_context * c = flow_context_create(); if (c == NULL) return 1;


//We construct a frame graph using numeric placeholders for input and output. 
FrameGraph * g = FrameGraph_create(c, 4096); //initial allocation size. failure to allocate enough space will ? cause a panic. ?
if (g == NULL){ return 2 }; //OOM 

int last = FrameNode_create(c, g,  NULL, FrameNode_Input_Placeholder, 0 );
last = FrameNode_create(c,g, last, FrameNode_Constrain, 300,200, Constrain_max, Upscale_canvas);
last = FrameNode_create(c,g, last, FrameNode_Output_Placeholder,0);




//Once the context is created, we rely on the fact that calls should fail via result code
ImageSource * image_a = ImageSource_create_and_copy_from_buffer(c, image_a_buffer, image_a_bytes); 
ImageSource_add_peek_function(c,image_a, peek_bytes, NULL);

ImageSource * image_b = ImageSource_create_empty(c); 
ImageSource_add_buffer_reader(c, image_b, image_b_buffer, image_b_uuid);

ImageSource * image_c = ImageSource_create_empty(c);
ImageSource_add_io(c, image_c, io, /* file ptr */);


//Wait, is it easier to run a binary search over input image sizes, or to implement constraint algebra over the graph? Or can the former solve for more than 1 variable?
ImageSource * image_simulation = ImageSource_create_with_dimensions(c, 200,100, flow_bgra32);

ImageJob * sim = ImageJob_create(c);
int useful_width;
int useful_height;
ImageJob_find_maximum_useful_dimensions(c, sim, g, &useful_width, &useful_height); 

ImageJob * job = ImageJob_create(c);

//coder and decoder instances are local to the image jobs

if (flow_context_has_error(c)){
    //TODO: propagate error details
    Context_destroy(c);
    return 1;
}

ImageJob_add_primary_source(c, job, image_a); //These can be called with null ImageSource, 
ImageJob_add_secondary_source(c, job, image_b);
ImageJob_add_target(c, job, image_c);

flow_status_code result = ImageJob_read_sources_info(c, job);
if (result == Ok){
    ImageSource_get_frame_count(c,image_a);
    ImageSource_get_page_count(c,image_a);
    ImageSource_get_dimensions(c,image_a, &w, &h);
    ImageJob_set_target_format(c, job, image_c, Jpeg, 90);
    ImageJob_autoset_target_format( c, job, image_c) ; //perhaps based on the source images?
    
    
    
    do {
    FrameGraph  * frame0 = FrameGraph_copy(c,g);
        ImageJob_complete_frame_graph(c, job, frame0);
        ImageGraph_flatten(c, job, frame0);
        ImageGraph_optimize(c, job, frame0);
        
    ImageJob_execute_all_targets(c, job, frame0);
    }while(ImageJob_next_frame(c,job));
    
        
}else{
    //Deal with error
    ImageJob_destroy(c, job);
    Context_destroy(c); //Destroying the context should ensure any ImageSource caches are freed. 
    return 2;
}
 

//If using a managed language, make sure you pin your reader/writer structs & functions.
ImageJob_destroy(c, job);
flow_context_destroy(c); //Destroying the context should ensure any ImageSource caches are freed. 
return 0;

```


