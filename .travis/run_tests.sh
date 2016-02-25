mkdir -p build
cd build
conan install --file ../conanfile_testing.txt --build missing
cmake ../ -DENABLE_TEST=ON && cmake --build . && ctest -V
