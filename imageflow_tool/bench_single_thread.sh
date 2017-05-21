#!/bin/bash

echo 'This benchmark is for ubuntu 14.04. '
echo 'Please run the following commands first for more accurate results'
echo mkdir bench_in
echo mkdir bench_out
echo sudo mount -t tmpfs -o size=512M tmpfs bench_in
echo sudo mount -t tmpfs -o size=512M tmpfs bench_out
echo
echo You may also need to sudo apt-get install parallel
echo and sudo apt-get install linux-tools-common linux-tools-generic
echo
echo on OS X, you will need to edit this script to use time instead of perf stat

cargo build --release
cp target/release/flow-proto1 .

convert --version
./flow-proto1 --version
vipsthumbnail --vips-version


wget -nc  https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/u1.jpg

mkdir bench_out
mkdir bench_in
rm bench_out/*.jpg
rm bench_in/*.jpg

export COUNT=1

for i in $(seq 1 $COUNT);
do
     cp "u1.jpg" "bench_in/c$i.jpg"
done


if [[ "$OSTYPE" == "linux-gnu" ]]; then
  export TIME_COMMAND="perf stat"
  export MEM_COMMAND="/usr/bin/time -v"
else
  export TIME_COMMAND=time
  export MEM_COMMAND=time
fi

export IMAGE_PATH=c1.jpg

cd bench_in || exit

echo Using imageflow to thumbnail
$TIME_COMMAND ../flow-proto1 -i $IMAGE_PATH -o ../bench_out/1_200x200.jpg -w 200 -h 200
$MEM_COMMAND ../flow-proto1 -i $IMAGE_PATH -o ../bench_out/1_200x200.jpg -w 200 -h 200
echo
echo
echo Using libvips to thumbnail
$TIME_COMMAND vipsthumbnail --linear --size=200x200  --output=../bench_out/1_vips_200x200.jpg $IMAGE_PATH
$MEM_COMMAND vipsthumbnail --linear --size=200x200  --output=../bench_out/1_vips_200x200.jpg  $IMAGE_PATH

echo
echo
echo Using ImageMagick to thumbnail
$TIME_COMMAND convert $IMAGE_PATH -limit thread 1 -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 200x200  -colorspace sRGB ../bench_out/1_magick_200x200.jpg
$MEM_COMMAND convert $IMAGE_PATH -limit thread 1 -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 200x200  -colorspace sRGB ../bench_out/1_magick_200x200.jpg
