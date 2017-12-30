

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

4. Reformat the c_components folder

clang-format -i c_components/{lib,tests}/*.{c,h,cpp,hpp}


5. Import code style .clion.codestyle.xml into CLion to reduce the number of differences clang-format creates.

## Using multiple versions of GCC

1. export CC=gcc-4.8
2. export CPP=g++-4.8


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



