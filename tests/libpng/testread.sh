#!/bin/bash
set -e #Exit on failure.

gcc -v


wget -nc https://sourceforge.net/projects/libpng/files/libpng16/older-releases/1.6.23/libpng-1.6.23.tar.gz

tar -xvzf libpng-1.6.23.tar.gz
cd libpng-1.6.23
CFLAGS="-fPIC" ./configure --prefix=$(pwd)/..
make
make install
cd ..
echo Compiling

gcc testread.c -lpng16 -lz -lm -static -L./lib -I./include -o testread.out


./testread.out

echo Running via Valgrind
valgrind ./testread.out --track-origins=yes


