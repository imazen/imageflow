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

(
cd bench_in || exit
hyperfine --export-markdown results.md --warmup 1 \
          'parallel "$HOME/bin/imageflow_tool v0.1/ir4 --in {} --out ../bench_out/{.}_200x200.jpg --command width=200&height=200&mode=max&quality=90" ::: *.jpg' \
          'parallel "vipsthumbnail --linear --size=200x200  --output=../bench_out/{.}_vips_200x200.jpg[Q=90] {}" ::: *.jpg' \
          'parallel "convert {} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 200x200  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_200x200.jpg" ::: *.jpg' \
          'parallel "convert {} -set colorspace sRGB -colorspace RGB -filter  Mitchell -distort Resize 200x200  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_ideal_200x200.jpg" ::: *.jpg'

echo "=============== Results in Markdown format ======================"
cat results.md
echo "================================================================="

hyperfine --export-markdown results.md --warmup 1 \
          'parallel "$HOME/bin/imageflow_tool v0.1/ir4 --in {} --out ../bench_out/{.}_2000x2000.jpg --command width=2000&height=2000&mode=max&quality=90" ::: *.jpg' \
          'parallel "vipsthumbnail --linear --size=2000x2000  --output=../bench_out/{.}_vips_2000x2000.jpg[Q=90] {}" ::: *.jpg' \
          'parallel "convert {} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 2000x2000  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_2000x2000.jpg" ::: *.jpg' \
          'parallel "convert {} -set colorspace sRGB -colorspace RGB -filter  Mitchell -distort Resize 2000x2000  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_ideal_2000x2000.jpg" ::: *.jpg'

echo "=============== Results in Markdown format ======================"
cat results.md
echo "================================================================="
)
