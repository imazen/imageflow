#!/bin/bash

./clean_dotfiles.sh

rm -rf artifacts
rm -rf build
rm -rf c_components/build
rm -rf target
rm libimageflow.so
rm c_components/conaninfo.txt
rm c_components/conanbuildinfo.cmake
rm *.user
rm c_components/conanfile.pyc
rm -rf node_frames
rm c_components/tests/visuals/compare*.png
rm c_components/tests/visuals/*.html
rm c_components/tests/visuals/*~
rm c_components/cacert.pem
rm -rf bin
rm *.png
rm *.jpg
rm *.jpeg
rm *.gif
rm *~

conan remove imageflow_c/* -f

./build.sh
