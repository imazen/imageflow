#!/bin/bash

set -e 
set -x

echo You may need to run
echo sudo apt-get install libgd-dev libpng libjpeg imagemagick
echo or brew install libgd libpng libjpeg imagemagick

#rm -rf resamplescope

if [ -e "./imageflow_tool" ]
then
  echo "Skipping compilation of imageflow, ./imageflow_tool exists. Delete to rebuild"
else
  cd ../..
  cargo build --release -p imageflow_tool_lib
  cd imageflow_tool/result_testing
  cp ../../target/release/imageflow_tool .
fi 

if [ -e "./rscope" ]
then
  echo "Skipping installation of resamplescope, ./rscope exists"
else
  rm -rf resamplescope
  git clone https://github.com/jsummers/resamplescope 
  cd resamplescope
  export CPATH=/usr/local/opt/gd/include:/usr/local/opt/libpng/include:/usr/local/opt/jpeg/include$CPATH
  export LIBRARY_PATH=/usr/local/opt/jpeg/lib:/usr/local/opt/libpng/lib:/usr/local/opt/gd/lib:$LIBRARY_PATH
  export LDFLAGS=" -L/usr/local/opt/jpeg/lib $LDFLAGS"
  export CPPFLAGS=" -I/usr/local/opt/jpeg/include $CPPFLAGS"
  export LDFLAGS=" -L/usr/local/opt/libpng/lib $LDFLAGS"
  export CPPFLAGS=" -I/usr/local/opt/libpng/include $CPPFLAGS"
  export LDFLAGS=" -L/usr/local/opt/gd/lib $LDFLAGS"
  export CPPFLAGS=" -I/usr/local/opt/gd/include $CPPFLAGS"


  make
  cd ..
  cp ./resamplescope/rscope ./rscope
fi

if [ -e "./imagew" ]
then
  echo "Skipping installation of image worsener, ./imagew exists"
else
  rm -rf imageworsener
  git clone https://github.com/jsummers/imageworsener
  cd imageworsener

  make -C scripts
  cd ..
  cp ./imageworsener/imagew ./imagew
fi 

#TODO: save a copy

# wget -nc http://www.imagemagick.org/download/binaries/ImageMagick-x86_64-apple-darwin15.4.0.tar.gz
# mkdir magick
# /usr/bin/gunzip -c ImageMagick-x86_64-apple-darwin15.4.0.tar.gz | /usr/bin/tar xf - -C magick
# mv magick/ImageMagick-7.0.1/ ./IM
# rm -rf ./magick
# export MAGICK_HOME="$(cd ..; pwd)/IM"
# export DYLD_LIBRARY_PATH="$MAGICK_HOME/lib/"
# export PATH="$MAGICK_HOME/bin:$PATH"

# wget -nc https://www.imagemagick.org/download/releases/ImageMagick-7.0.2-1.tar.gz

# tar xvzf ImageMagick-7.0.2-1.tar.gz
# cd ImageMagick-7.0.2-1
# ./configure --enable-hdri=yes --with-bzlib=no --with-djvu=no --with-dps=no --with-fftw=no --with-fpx=no --with-fftw=no --with-fontconfig=no --with-freetype=no --with-gvc=no --with-jbig=no --with-magick-plus-plus=no --with-pango=no
# export DESTDIR="$(cd ..; pwd)/bin"
# make
# make install
# ln -s ./bin/usr/local/bin/convert ../convert
# ln -s ./bin/usr/local/bin/identify ../identify
# cd ..

