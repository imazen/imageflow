mkdir -p build
cd build
conan install -u --file ../conanfile.py -o build_tests=True --build missing
cmake ../ -DENABLE_TEST=ON && cmake --build . && ctest -V
