#!/bin/bash
set -e

# Delete the profiling dir to re-install
(
if [ -d "profiling" ]; then
    cd profiling || exit
else
    mkdir profiling
    cd profiling || exit
    conan install --file ../conanfile.py --scope profiling=True --scope build_tests=False --build missing
fi
conan build --file ../conanfile.py
time build/bin/profile_imageflow
gprof build/bin/profile_imageflow gmon.out > ../profile.txt
)
(
cd build || exit
declare -a arr=("compositing" "render1d" "scaling" "color" "convolution" "codecs_jpeg_idct_fast" "scale2d")
for i in "${arr[@]}"
do
    make "lib/$i.s"
    as -alhnd "CMakeFiles/imageflow.dir/lib/$i.c.s" > "../lib/$i.c.s"
done
)
