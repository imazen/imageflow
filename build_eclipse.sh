
 mkdir ../eclipse_imageflow
 cd ../eclipse_imageflow
 conan install -u --file ../imageflow/conanfile.py -o build_tests=False -o profiling=True --build missing
 cmake -G"Eclipse CDT4 - Unix Makefiles" -DSKIP_LIBRARY=ON -DENABLE_TEST=OFF -DENABLE_PROFILING=ON ../imageflow
 cmake --build .
 cd ../imageflow
