# Delete the profiling dir to re-install

if [ -d "profiling" ]; then
    cd profiling
else
    mkdir profiling
    cd profiling
    conan install --file ../conanfile.py -o profiling=True -o build_tests=False --build missing
fi
conan build --file ../conanfile.py
time build/bin/profile_imageflow
gprof build/bin/profile_imageflow gmon.out > ../profile.txt
cd ..
cd build
declare -a arr=("compositing" "render1d" "scaling" "color" "convolution" "codecs_jpeg_idct_fast" "scale2d")
for i in "${arr[@]}"
do
    make "lib/$i.s"
    as -alhnd "CMakeFiles/imageflow.dir/lib/$i.c.s" > ../lib/$i.c.s
done
cd ..
