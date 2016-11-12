#!/bin/bash
set -e #Exit on failure.

gcc -v


wget -nc https://sourceforge.net/projects/libpng/files/libpng16/older-releases/1.6.23/libpng-1.6.23.tar.gz
wget -nc http://zlib.net/zlib128.zip
tar -xvzf libpng-1.6.23.tar.gz
cd libpng-1.6.23
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

gcc testread.c -lpng16 -lz -lm -static -L./lib -I./include -o testread.out
valgrind ./testread.out --track-origins=yes


