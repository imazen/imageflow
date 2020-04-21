#!/bin/bash

echo 'This benchmark is for ubuntu 20.04. '

$HOME/bin/imageflow_tool --version
convert --version
vipsthumbnail --vips-version


#wget -nc --quiet  https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/u1.jpg


mkdir bench_out
mkdir bench_in
rm bench_out/*.jpg
rm bench_in/*.jpg

export COUNT=32

for i in $(seq 1 $COUNT);
do
     cp "u1.jpg" "bench_in/c$i.jpg"
done

# Can't get perf stat on ubuntu 20.04 for some reason
#if [[ "$OSTYPE" == "linux-gnu" ]]; then
#  export TIME_COMMAND="perf stat"
#else
  export TIME_COMMAND=time
#fi

(
cd bench_in || exit



echo Using imageflow to thumbnail $COUNT images in parallel
$TIME_COMMAND parallel '$HOME/bin/imageflow_tool v0.1/ir4 --in {} --out ../bench_out/{.}_200x200.jpg --command width=200&height=200&quality=90' ::: *.jpg
echo
echo
echo Using libvips to thumbnail $COUNT images in parallel
$TIME_COMMAND parallel 'vipsthumbnail --linear --size=200x200  --output=../bench_out/{.}_vips_200x200.jpg[Q=90] {}' ::: *.jpg

echo
echo
echo Using ImageMagick to thumbnail $COUNT images in parallel
$TIME_COMMAND parallel 'convert {} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 200x200  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_200x200.jpg' ::: *.jpg

echo
echo
echo Using ImageMagick ideal settings to thumbnail $COUNT images in parallel
$TIME_COMMAND parallel 'convert {} -set colorspace sRGB -colorspace RGB -filter  Mitchell -distort Resize 200x200  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_ideal_200x200.jpg' ::: *.jpg


echo
echo
echo Using imageflow to create 2000px versions of $COUNT images in parallel
$TIME_COMMAND parallel '$HOME/bin/imageflow_tool v0.1/ir4 --in {} --out ../bench_out/{.}_2000x2000.jpg --command width=2000&height=2000&quality=90' ::: *.jpg
echo
echo
echo Using libvips to create 2000px versions of $COUNT images in parallel
$TIME_COMMAND parallel 'vipsthumbnail --linear --size=2000x2000  --output=../bench_out/{.}_vips_2000x2000.jpg[Q=90] {}' ::: *.jpg

echo
echo
echo Using ImageMagick to create 2000px versions of $COUNT images in parallel
$TIME_COMMAND parallel 'convert {} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 2000x2000  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_2000x2000.jpg' ::: *.jpg

echo
echo
echo Using ImageMagick ideal settings to create 2000px versions of $COUNT images in parallel
$TIME_COMMAND parallel 'convert {} -set colorspace sRGB -colorspace RGB -filter  Mitchell -distort Resize 2000x2000  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_ideal_2000x2000.jpg' ::: *.jpg


)
