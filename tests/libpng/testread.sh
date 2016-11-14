#!/bin/bash
set -e #Exit on failure.

gcc -v

VER=1.6.26
FOLDER=/${VER}
#FOLDER=/older-releases/${VER}

wget -nc https://sourceforge.net/projects/libpng/files/libpng16${FOLDER}/libpng-${VER}.tar.gz
wget -nc http://zlib.net/zlib128.zip
tar -xvzf libpng-${VER}.tar.gz
cd libpng-${VER}
CFLAGS="-fPIC" ./configure --prefix=$(pwd)/..
make
make install
cd ..
unzip zlib128.zip
cd zlib-1.2.8
CFLAGS="-fPIC" ./configure --prefix=$(pwd)/..
make
make install
cd ..

#static require to reproduce the problem

gcc testread.c -lpng16 -lz -lm -static -L./lib -I./include -o testread.out

#Helps if we're using the shared libs
export LD_LIBRARY_PATH=./lib:${LD_LIBRARY_PATH}

valgrind ./testread.out --track-origins=yes


