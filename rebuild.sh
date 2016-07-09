#!/bin/bash
rm -rf build
rm libimageflow.so
rm conaninfo.txt
rm conanbuildinfo.cmake
rm *.user
rm conanfile.pyc
rm -rf node_frames
rm tests/visuals/compare*.png
rm tests/visuals/*.html
rm tests/visuals/*~
rm cacert.pem
rm -rf bin
rm *.png
rm *.jpg
rm *.jpeg
rm *.gif
rm *~

mkdir -p build
cd build
conan install --scope build_tests=True --build missing -u ../
conan build ../

cd ..
conan remove imageflow/* -f
conan export lasote/testing

cd wrappers/server

conan install --build missing # Will build imageflow package with your current settings
cargo build
cargo test

cd ../..
