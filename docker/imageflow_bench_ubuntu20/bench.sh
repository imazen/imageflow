#!/bin/bash

echo 'This benchmark is for ubuntu 20.04. '

"$HOME/bin/imageflow_tool" --version
convert --version
vipsthumbnail --vips-version

#wget -nc --quiet  https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/u1.jpg

mkdir bench_out
mkdir bench_in
rm bench_out/*.jpg
rm bench_in/*.jpg

export COUNT=32

for i in $(seq 1 $COUNT); do
  cp "u1.jpg" "bench_in/c$i.jpg"
done

cd bench_in

if [[ "$1" == "thumbnail" ]]; then
  # shellcheck disable=SC2016
  hyperfine --export-markdown results.md \
    'parallel "$HOME/bin/imageflow_tool v0.1/ir4 --in {} --out ../bench_out/{.}_200x200.jpg --command width=\"200&height=200&mode=max&quality=90&jpeg.turbo=true\"" ::: *.jpg' \
    'parallel "vipsthumbnail --linear --size=200x200  --output=../bench_out/{.}_vips_200x200.jpg[Q=90,strip] {}" ::: *.jpg' \
    'parallel "gm convert {} -set colorspace sRGB -colorspace RGB -filter Mitchell -resize 200x200  -colorspace sRGB -quality 90 ../bench_out/{.}_gm_200x200.jpg" ::: *.jpg' \
    'parallel "convert {} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 200x200  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_200x200.jpg" ::: *.jpg'

  echo "=============== Results in Markdown format ======================"
  cat results.md
  echo "================================================================="
fi
if [[ "$1" == "downscale" ]]; then
  # shellcheck disable=SC2016
  hyperfine --export-markdown results.md \
    'parallel "$HOME/bin/imageflow_tool v0.1/ir4 --in {} --out ../bench_out/{.}_2000x2000.jpg --command width=\"2000&height=2000&mode=max&quality=90&jpeg.turbo=true\"" ::: *.jpg' \
    'parallel "vipsthumbnail --linear --size=2000x2000  --output=../bench_out/{.}_vips_2000x2000.jpg[Q=90] {}" ::: *.jpg' \
    'parallel "gm convert {} -set colorspace sRGB -colorspace RGB -filter Mitchell -resize 2000x2000  -colorspace sRGB -quality 90 ../bench_out/{.}_gm_0200x0200.jpg" ::: *.jpg' \
    'parallel "convert {} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 2000x2000  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_2000x2000.jpg" ::: *.jpg'

  echo "=============== Results in Markdown format ======================"
  cat results.md
  echo "================================================================="
fi

if [[ "$1" == "jpegsize" ]]; then

  "$HOME/bin/imageflow_tool" v0.1/ir4 --in c1.jpg --out ../bench_out/imageflow_2000x2000.jpg --command "width=2000&height=2000&mode=max&quality=89" 1>/dev/null
  vipsthumbnail --linear --size=2000x2000  --output=../bench_out/vips_2000x2000.jpg[Q=89,strip,optimize-coding] c1.jpg
  convert c1.jpg -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 2000x2000  -colorspace sRGB -quality 89 ../bench_out/magick_2000x2000.jpg

  "$HOME/bin/imageflow_tool" v0.1/ir4 --in c1.jpg --out ../bench_out/imageflow_reference_2000x2000.png --command "width=2000&height=2000&mode=max&format=png" 1>/dev/null
  vipsthumbnail --linear --size=2000x2000  --output=../bench_out/vips_reference_2000x2000.png c1.jpg
  convert c1.jpg -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 2000x2000  -colorspace sRGB ../bench_out/magick_reference_2000x2000.png

  "$HOME/bin/imageflow_tool" v0.1/ir4 --in ../bench_out/imageflow_2000x2000.jpg --out ../bench_out/imageflow_2000x2000.png --command "format=png" 1>/dev/null
  "$HOME/bin/imageflow_tool" v0.1/ir4 --in ../bench_out/vips_2000x2000.jpg --out ../bench_out/vips_2000x2000.png --command "format=png" 1>/dev/null
  "$HOME/bin/imageflow_tool" v0.1/ir4 --in ../bench_out/magick_2000x2000.jpg --out ../bench_out/magick_2000x2000.png --command "format=png" 1>/dev/null

  echo "=============== DSSIM relative to imageflow reference (lower is better)  ======================"
  dssim ../bench_out/imageflow_reference_2000x2000.png ../bench_out/imageflow_2000x2000.png
  dssim ../bench_out/imageflow_reference_2000x2000.png ../bench_out/vips_2000x2000.png
  dssim ../bench_out/imageflow_reference_2000x2000.png ../bench_out/magick_2000x2000.png

  echo "=============== DSSIM relative to libvips reference (lower is better)  ======================"
  dssim ../bench_out/vips_reference_2000x2000.png ../bench_out/imageflow_2000x2000.png
  dssim ../bench_out/vips_reference_2000x2000.png ../bench_out/vips_2000x2000.png
  dssim ../bench_out/vips_reference_2000x2000.png ../bench_out/magick_2000x2000.png

  echo "=============== DSSIM relative to ImageMagick reference (lower is better)  ======================"
  dssim ../bench_out/magick_reference_2000x2000.png ../bench_out/imageflow_2000x2000.png
  dssim ../bench_out/magick_reference_2000x2000.png ../bench_out/vips_2000x2000.png
  dssim ../bench_out/magick_reference_2000x2000.png ../bench_out/magick_2000x2000.png

  "$HOME/bin/imageflow_tool" v0.1/ir4 --in c1.jpg --out ../bench_out/imageflow_idct_scaling_2000x2000.png --command "width=2000&height=2000&mode=max&format=png" 1>/dev/null
  "$HOME/bin/imageflow_tool" v0.1/ir4 --in c1.jpg --out ../bench_out/imageflow_no_idct_scaling_2000x2000.png --command "width=2000&height=2000&mode=max&format=png&decoder.min_precise_scaling_ratio=100" 1>/dev/null

  echo "=============== DSSIM with linear IDCT scaling turned on vs. no IDCT scaling  ======================"
  dssim ../bench_out/imageflow_no_idct_scaling_2000x2000.png ../bench_out/imageflow_idct_scaling_2000x2000.png



  echo "=============== File sizes ======================"
  ls -l ../bench_out
  echo "================================================================="
  # shellcheck disable=SC2028
  echo "To see results, run docker run -v %CD%\results:/home/imageflow/bench_out imazen/imageflow_bench_ubuntu20 jpegsize"
  echo 'or on linux,  docker run -v \"$(pwd)\"/results:/home/imageflow/bench_out imazen/imageflow_bench_ubuntu20 jpegsize'

fi

