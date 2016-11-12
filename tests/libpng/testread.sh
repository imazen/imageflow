#!/bin/bash
set -e #Exit on failure.

gcc -v


#wget -nc https://sourceforge.net/projects/libpng/files/libpng16/older-releases/1.6.23/libpng-1.6.23.tar.gz

#tar -xvzf libpng-1.6.23.tar.gz
#cd libpng-1.6.23.tar.gz

conan install

echo Compiling

gcc testread.c -g @conanbuildinfo.gcc  -o testread.out

#-fsanitize=address

./testread.out

echo Running via Valgrind
valgrind ./testread.out --track-origins=yes


