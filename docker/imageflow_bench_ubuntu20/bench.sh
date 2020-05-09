#!/bin/bash

echo 'This benchmark is for ubuntu 20.04. '

"$HOME/bin/imageflow_tool" --version
convert --version
vipsthumbnail --vips-version


mkdir bench_out
mkdir bench_in
sudo chmod -R a+rwx bench_out
rm -rf bench_out/**
rm -rf bench_in/**

export COUNT=32

for i in $(seq 1 $COUNT); do
  cp "u1.jpg" "bench_in/c$i.jpg"
done

cd bench_in

if [[ "$1" == "thumbnail" ]]; then

  # shellcheck disable=SC2016
  hyperfine --export-markdown results.md  --warmup 1 \
    'parallel "$HOME/bin/imageflow_tool v1/querystring --in {} --quiet --out ../bench_out/{.}_200x200.jpg --command width=200&height=200&mode=max&quality=100" ::: *.jpg' \
    'parallel "vipsthumbnail --linear --size=200x200  --output=../bench_out/{.}_vips_200x200.jpg[Q=90,strip] {}" ::: *.jpg' \
    'parallel "gm convert {} -set colorspace sRGB -colorspace RGB -filter Mitchell -resize 200x200  -colorspace sRGB -quality 90 ../bench_out/{.}_gm_200x200.jpg" ::: *.jpg' \
    'parallel "convert {} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 200x200  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_200x200.jpg" ::: *.jpg'

  echo "=============== Results in Markdown format ======================"
  cat results.md
  echo "================================================================="
fi
if [[ "$1" == "downscale" ]]; then
  # shellcheck disable=SC2016
  hyperfine --export-markdown results.md  --warmup 1 \
    'parallel "$HOME/bin/imageflow_tool v1/querystring --in {}  --quiet --out ../bench_out/{.}_2000x2000.jpg --command width=2000&height=2000&mode=max&quality=90" ::: *.jpg' \
    'parallel "vipsthumbnail --linear --size=2000x2000  --output=../bench_out/{.}_vips_2000x2000.jpg[Q=90] {}" ::: *.jpg' \
    'parallel "convert {} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 2000x2000  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_2000x2000.jpg" ::: *.jpg'

# Can't get graphicsmagick to work
#  'parallel --show-output "gm convert {} -set colorspace sRGB -colorspace RGB -filter Mitchell -resize 2000x2000  -colorspace sRGB -quality 90 ../bench_out/{.}_gm_2000x2000.jpg" ::: *.jpg' \
  echo "=============== Results in Markdown format ======================"
  cat results.md
  echo "================================================================="
fi

if [[ "$1" == "jpegsize" ]]; then

  "$HOME/bin/imageflow_tool" v1/querystring  --quiet --in c1.jpg --out ../bench_out/imageflow_2000x2000.jpg --command "width=2000&height=2000&mode=max&quality=89"
  vipsthumbnail --linear --size=2000x2000  --output=../bench_out/vips_2000x2000.jpg[Q=89,strip,optimize-coding] c1.jpg
  convert c1.jpg -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 2000x2000  -colorspace sRGB -quality 89 ../bench_out/magick_2000x2000.jpg

  "$HOME/bin/imageflow_tool" v1/querystring  --quiet --in c1.jpg --out ../bench_out/imageflow_reference_2000x2000.png --command "width=2000&height=2000&mode=max&format=png"
  vipsthumbnail --linear --size=2000x2000  --output=../bench_out/vips_reference_2000x2000.png c1.jpg
  convert c1.jpg -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 2000x2000  -colorspace sRGB ../bench_out/magick_reference_2000x2000.png

  "$HOME/bin/imageflow_tool" v1/querystring --quiet --in ../bench_out/imageflow_2000x2000.jpg --out ../bench_out/imageflow_2000x2000.png --command "format=png"
  "$HOME/bin/imageflow_tool" v1/querystring --quiet --in ../bench_out/vips_2000x2000.jpg --out ../bench_out/vips_2000x2000.png --command "format=png"
  "$HOME/bin/imageflow_tool" v1/querystring --quiet --in ../bench_out/magick_2000x2000.jpg --out ../bench_out/magick_2000x2000.png --command "format=png"

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

  "$HOME/bin/imageflow_tool" v1/querystring --quiet --in c1.jpg --out ../bench_out/imageflow_idct_scaling_2000x2000.png --command "width=2000&height=2000&mode=max&format=png"
  "$HOME/bin/imageflow_tool" v1/querystring --quiet --in c1.jpg --out ../bench_out/imageflow_no_idct_scaling_2000x2000.png --command "width=2000&height=2000&mode=max&format=png&decoder.min_precise_scaling_ratio=100"

  echo "=============== DSSIM with linear IDCT scaling turned on vs. no IDCT scaling  ======================"
  dssim ../bench_out/imageflow_no_idct_scaling_2000x2000.png ../bench_out/imageflow_idct_scaling_2000x2000.png

  echo "=============== File sizes ======================"
  ls -l -S ../bench_out
  echo "================================================="

fi


if [[ "$1" == "pngsize" ]]; then

pngsize(){

  mkdir ../bench_out/$2
  cp $1 ../bench_out/$2/$2_original.png
  echo "Running compression tools"
   "$HOME/bin/imageflow_tool" v1/querystring  --quiet --in $1 --out ../bench_out/$2/$2_expanded.png --command "format=png"

   "$HOME/bin/imageflow_tool" v1/querystring  --quiet --in ../bench_out/$2/$2_expanded.png --out ../bench_out/$2/$2_imageflow.png --command "format=png&png.max_deflate=true&png.libpng=true"
   "$HOME/bin/imageflow_tool" v1/querystring  --quiet --in ../bench_out/$2/$2_expanded.png --out ../bench_out/$2/$2_imageflow_fast.png --command "format=png"
   "$HOME/bin/imageflow_tool" v1/querystring  --quiet --in ../bench_out/$2/$2_expanded.png --out ../bench_out/$2/$2_imageflow_lossy_fast.png --command "format=png&png.min_quality=0&png.quality=80"
   "$HOME/bin/imageflow_tool" v1/querystring  --quiet --in ../bench_out/$2/$2_expanded.png --out ../bench_out/$2/$2_imageflow_lossy.png --command "format=png&png.max_deflate=true&png.min_quality=0&png.quality=80"
   convert ../bench_out/$2/$2_expanded.png -set colorspace sRGB -colorspace RGB -colorspace sRGB ../bench_out/$2/$2_magick.png
   pngcrush -q ../bench_out/$2/$2_expanded.png ../bench_out/$2/$2_pngcrush.png
   cp ../bench_out/$2/$2_expanded.png ../bench_out/$2/$2_oxipng.png
   oxipng -q -o 4 --strip safe ../bench_out/$2/$2_oxipng.png


  echo "=============== DSSIM relative to original (lower is better)  ============="
  dssim ../bench_in/$1 ../bench_out/$2/$2_imageflow.png
  dssim ../bench_in/$1 ../bench_out/$2/$2_imageflow_fast.png
  dssim ../bench_in/$1 ../bench_out/$2/$2_imageflow_lossy.png
  dssim ../bench_in/$1 ../bench_out/$2/$2_imageflow_lossy_fast.png
  dssim ../bench_in/$1 ../bench_out/$2/$2_pngcrush.png
  dssim ../bench_in/$1 ../bench_out/$2/$2_magick.png
  dssim ../bench_in/$1 ../bench_out/$2/$2_oxipng.png

  echo "=============== File sizes ================================================="
  ls -l -S ../bench_out/$2
  echo "============================================================================"

}
   echo "Downloading test files..."
   wget -nc --quiet  https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/rose.png
   wget -nc --quiet  https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/dice.png
   wget -nc --quiet  https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/compass.png
   wget -nc --quiet  https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/u1.jpg
   wget -nc --quiet  https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/u6.jpg
   wget -nc --quiet  https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg


   "$HOME/bin/imageflow_tool" v1/querystring  --quiet --in u1.jpg --out van.png --command "format=png&width=800"
   "$HOME/bin/imageflow_tool" v1/querystring  --quiet --in u6.jpg --out pier.png --command "format=png&width=800"
   "$HOME/bin/imageflow_tool" v1/querystring  --quiet --in waterhouse.jpg --out waterhouse.png --command "format=png&width=800"

    pngsize "waterhouse.png" "waterhouse"
    pngsize "dice.png" "dice"
    pngsize "rose.png" "rose"
    pngsize "van.png" "van"
    pngsize "pier.png" "pier"

fi

sudo chmod -R a+rwx ../bench_out/

# shellcheck disable=SC2028
echo "To see results, run docker run -v %CD%\results:/home/imageflow/bench_out imazen/imageflow_bench_ubuntu20 jpegsize"
echo 'or on linux,  docker run -v "$(pwd)"/results:/home/imageflow/bench_out imazen/imageflow_bench_ubuntu20 jpegsize'
