
# Compiling ImageMagick for high quality

**We use 6.9.3-7 - the latest stable as of April 2016.**

See http://www.imagemagick.org/script/install-source.php

And http://www.imagemagick.org/script/high-dynamic-range.php

```bash

#download and utar
wget http://www.imagemagick.org/download/ImageMagick-6.9.3-7.tar.gz
tar xvzf ImageMagick-6.9.3-7.tar.gz
cd ImageMagick-6.9.3-7
# Remove your existing imagemagick so you don't use it accidentally and generate crappy images. 
# Older and differently compiled versions of imagemagick have lots of bugs, and are roughly MS Paint quality.
sudo apt-get remove imagemagick

# Configure for HDRI support (don't truncate color depth)
./configure --enable-hdri --with-modules
#Build
make
#Install
sudo make install

#Verify the output lists HDRI and the version matches
convert -v

```

If you get this error:

> convert: error while loading shared libraries: libMagickCore-6.Q16HDRI.so.2: cannot open shared object file: No such file or directory

Then run `ldconfig /usr/local/lib`

If you need to remove the binaries installed by `make install`, try

> sudo rm /usr/local/bin/{animate,compare,conjure,convert,composite,identify,display,import,mogrify,montage,stream}

# Usage

# Best general-purpose downsampling

convert INPUT.IMG -set colorspace WHATEVER_YOUR_INPUT_FITS_IN -colorspace RGB -filter Mitchell -distort Resize RESIZE_SPECIFICATION -colorspace YOUR_CHOSEN_OUTPUT_COLORSPACE OUTPUT.IMG

convert u1.jpg -set colorspace sRGB -colorspace RGB -filter Mitchell -distort Resize 800x800 -colorspace sRGB imagemagick_u1_800x800.jpg

# Best general-purpose upscaling

same replacing -filter Mitchell -distort Resize with -gamma 2 -filter LanczosSharp -distort Resize -gamma .5
