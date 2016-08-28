#!/bin/bash


rm -rf artifacts
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

conan remove imageflow/* -f

./build.sh
