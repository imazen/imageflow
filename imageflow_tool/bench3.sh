#!/bin/bash

./build_release_tool.sh

vipsthumbnail --vips-version

wget -nc  https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/u1.jpg

mkdir bench_out
mkdir bench_in
rm bench_out/*.jpg
rm bench_in/*.jpg

export COUNT=8

for i in $(seq 1 $COUNT);
do
     cp "u1.jpg" "bench_in/c$i.jpg"
done

if [[ "$OSTYPE" == "linux-gnu" ]]; then
  export TIME_COMMAND="perf stat"
else
  export TIME_COMMAND=time
fi

(
cd bench_in || exit
#
echo Using flow-proto1 to thumbnail $COUNT images in parallel
$TIME_COMMAND parallel '../flow-proto1 --min_precise_scaling_ratio 1 -i {} -o ../bench_out/{.}_200x200.jpg -w 200 -h 200 --jpeg-quality 65 --format jpg' ::: *.jpg
#
echo
echo
echo Using imageflow_tool to thumbnail $COUNT images in parallel
$TIME_COMMAND parallel '../imageflow_tool v0.1/ir4 --in {} --out ../bench_out/{.}_200x200.jpg --command "maxwidth=200&maxheight=200&quality=65&format=jpg" > /dev/null' ::: *.jpg
#
echo
echo
echo Using libvips to thumbnail $COUNT images in parallel
$TIME_COMMAND parallel 'vipsthumbnail --linear --size=200x200 --format=jpg --output=../bench_out/{.}_vips_200x200.jpg[Q=65] {}' ::: *.jpg
)