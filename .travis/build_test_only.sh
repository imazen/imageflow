mkdir -p build
cd build
cmake ../ -DENABLE_TEST=ON && cmake --build . && ctest -V
cd ..