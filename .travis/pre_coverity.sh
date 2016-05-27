#!/bin/bash

sudo apt-get install nasm
sudo apt-get install -y pkg-config libpng12-dev
sudo pip install conan --upgrade

#Download and build DSSIM
wget https://github.com/pornel/dssim/archive/master.tar.gz
tar xvzf master.tar.gz
cd dssim-master
make
cd bin
export PATH=$PATH:$(pwd)
cd ../..
pwd

mkdir build && cd build
conan install  -o build_tests=False --build missing -u ..
cd ..
