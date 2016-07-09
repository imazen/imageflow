#!/bin/bash

convert --version
./flow-proto1 --version
./imagew --version

mkdir turtles
rm ./turtles/*.jpg
rm ./turtles/*.png

if [[ "$OSTYPE" == "linux-gnu" ]]; then
  export MEM_COMMAND="/usr/bin/time -v"
else
  export MEM_COMMAND=time
fi

#Reduce noise for now
export MEM_COMMAND=

./fetch_images.sh

cd turtles

export IMAGE_PATH=../source_images/turtleegglarge.jpg

echo Using imageflow linearly
$MEM_COMMAND ../flow-proto1 -i $IMAGE_PATH -o flow_linear_s0_400x400.png -w 400 -h 400 --format png24 -m 100 --sharpen 0 --down-filter bspline --up-filter bspline 

echo
echo Using ImageMagick to thumbnail
$MEM_COMMAND convert $IMAGE_PATH -set colorspace sRGB -colorspace RGB -filter bspline -resize 400x400  -colorspace sRGB  magick_linear_400x400.png

echo
echo Using ImageWorsener to thumbnail
$MEM_COMMAND ../imagew -filter bspline  $IMAGE_PATH -w 400   imagew_linear_400x400.png

echo Using imageflow wrong
$MEM_COMMAND ../flow-proto1 -i $IMAGE_PATH -o flow_wrong_400x400.png -w 400 -h 400 --format png24 --incorrectgamma -m 100 --down-filter bspline --up-filter bspline 

echo
echo Using ImageMagick to thumbnail wrong
$MEM_COMMAND convert $IMAGE_PATH -filter bspline -resize 400x400 magick_wrong_400x400.png

echo
echo Using ImageWorsener with -nogamma
$MEM_COMMAND ../imagew -nogamma -filter bspline  $IMAGE_PATH -w 400    imagew_wrong_400x400.png


echo Comparing linear results without sharpening

dssim magick_linear_400x400.png flow_linear_s0_400x400.png
dssim imagew_linear_400x400.png flow_linear_s0_400x400.png


echo Comparing gamma-incorrect results

dssim flow_wrong_400x400.png magick_wrong_400x400.png

identify -verbose magick_wrong_400x400.png > wrong_im.txt

identify -verbose flow_wrong_400x400.png > wrong_flow.txt
diff wrong_im.txt wrong_flow.txt | grep Gamma

compare flow_linear_s0_400x400.png magick_linear_400x400.png  -fuzz 0.5% compare_linear_fuzz_05.png
compare flow_linear_s0_400x400.png magick_linear_400x400.png  -fuzz 1% compare_linear_fuzz_1.png

compare flow_wrong_400x400.png magick_wrong_400x400.png  -fuzz 0.5% compare_wrong_fuzz_05.png
compare flow_wrong_400x400.png magick_wrong_400x400.png  -fuzz 1% compare_wrong_fuzz_1.png

compare flow_linear_s0_400x400.png imagew_linear_400x400.png  -fuzz 0.5% imagew_compare_linear_fuzz_05.png
compare flow_linear_s0_400x400.png imagew_linear_400x400.png  -fuzz 1% imagew_compare_linear_fuzz_1.png

compare flow_wrong_400x400.png imagew_wrong_400x400.png  -fuzz 0.5% imagew_compare_wrong_fuzz_05.png
compare flow_wrong_400x400.png imagew_wrong_400x400.png  -fuzz 1% imagew_compare_wrong_fuzz_1.png


cd ..

firefox ./turtles || open ./turtles
