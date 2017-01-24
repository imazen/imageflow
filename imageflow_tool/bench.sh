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
echo And run brew install parallel

#-limit thread 1

RUSTFLAGS="-C target-cpu=native" cargo build --release
cp target/release/flow-proto1 .

convert --version
./flow-proto1 --version
vipsthumbnail --vips-version


wget -nc  https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/u1.jpg


mkdir bench_out
mkdir bench_in
rm bench_out/*.jpg
rm bench_in/*.jpg


export COUNT=32


for i in $(seq 1 $COUNT);
do
     cp "u1.jpg" "bench_in/c$i.jpg"
done




if [[ "$OSTYPE" == "linux-gnu" ]]; then
  export TIME_COMMAND="perf stat"
else
  export TIME_COMMAND=time
fi

cd bench_in

echo Using imageflow to thumbnail $COUNT images in parallel
$TIME_COMMAND parallel '../flow-proto1 -i {} -o ../bench_out/{.}_200x200.jpg -w 200 -h 200' ::: *.jpg
echo
echo
echo Using libvips to thumbnail $COUNT images in parallel
$TIME_COMMAND parallel 'vipsthumbnail --linear --size=200x200  --output=../bench_out/{.}_vips_200x200.jpg {}' ::: *.jpg

echo
echo
echo Using ImageMagick to thumbnail $COUNT images in parallel
$TIME_COMMAND parallel 'convert {} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 200x200  -colorspace sRGB ../bench_out/{.}_magick_200x200.jpg' ::: *.jpg

echo
echo
echo Using ImageMagick ideal settings to thumbnail $COUNT images in parallel
$TIME_COMMAND parallel 'convert {} -set colorspace sRGB -colorspace RGB -filter  Mitchell -distort Resize 200x200  -colorspace sRGB ../bench_out/{.}_magick_ideal_200x200.jpg' ::: *.jpg


echo
echo
echo Using imageflow to create 2000px versions of $COUNT images in parallel
$TIME_COMMAND parallel '../flow-proto1 -i {} -o ../bench_out/{.}_2000x2000.jpg -w 2000 -h 2000' ::: *.jpg
echo
echo
echo Using libvips to create 2000px versions of $COUNT images in parallel
$TIME_COMMAND parallel 'vipsthumbnail --linear --size=2000x2000  --output=../bench_out/{.}_vips_2000x2000.jpg {}' ::: *.jpg

echo
echo
echo Using ImageMagick to create 2000px versions of $COUNT images in parallel
$TIME_COMMAND parallel 'convert {} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 2000x2000  -colorspace sRGB ../bench_out/{.}_magick_2000x2000.jpg' ::: *.jpg

echo
echo
echo Using ImageMagick ideal settings to create 2000px versions of COUNT images in parallel
$TIME_COMMAND parallel 'convert {} -set colorspace sRGB -colorspace RGB -filter  Mitchell -distort Resize 2000x2000  -colorspace sRGB ../bench_out/{.}_magick_ideal_2000x2000.jpg' ::: *.jpg


echo
echo
echo Using imageflow wrong on $COUNT images in parallel. 200x200
$TIME_COMMAND parallel '../flow-proto1 -i {} --incorrectgamma -o ../bench_out/{.}_200x200_wrong.jpg -w 200 -h 200' ::: *.jpg
echo
echo
echo Using libvips wrong on $COUNT images in parallel. 200x200
$TIME_COMMAND parallel 'vipsthumbnail --size=200x200  --output=../bench_out/{.}_vips_200x200_wrong.jpg {}' ::: *.jpg

echo
echo
echo Using ImageMagick wrong on $COUNT images in parallel. 200x200
$TIME_COMMAND parallel 'convert {} -filter Robidoux -resize 200x200 ../bench_out/{.}_magick_200x200_wrong.jpg' ::: *.jpg

cd ..
