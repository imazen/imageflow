#!/bin/bash
# shellcheck disable=SC2086

# we WANT word-splitting behavior for our substitutions below

echo Run install_tools.sh first if you have any issues


convert --version
./imageflow_tool --version
./imagew --version

mkdir scope
cd scope 

rm ./*.png
rm ./index.html

../rscope -gen
../rscope -r -gen

rm ./rscope*.htm*

declare -a arr=("robidoux" "robidoux_sharp" "ginseng" "cubic_b_spline" "hermite" "lanczos" "n_cubic" "n_cubic_sharp" "triangle" "box" "mitchell" "catmull_rom")

# TODO - test the following
#robidoux_fast - A faster, less accurate version of robidoux
#ginseng_sharp
#lanczos_sharp
#lanczos_2
#lanczos_2_sharp
#cubic
#cubic_sharp
#jinc
#fastest

## now loop through the above array
for i in "${arr[@]}"
do

  m="-filter ${i}"
  w="-filter ${i}"
  f="${i}"
  if [ "${i}" = "ginseng" ]; then
    m=" -define filter:filter=Sinc -define filter:window=Jinc -define filter:lobes=3 "

  fi
  if [ "${i}" = "robidoux" ]; then
    w=" -filter cubic0.37821575509399867,0.31089212245300067 "
  fi
  if [ "${i}" = "robidoux_sharp" ]; then
    w=" -filter cubic0.2620145123990142,0.3689927438004929 "
    m=" -filter robidouxsharp "
  fi
  if [ "${i}" = "cubic_b_spline" ]; then
    w=" -filter bspline "
    m=" -filter spline "
  fi

  if [ "${i}" = "catmull_rom" ]; then
    w=" -filter catrom "
    m=" -filter catrom "
  fi

  if [ "${i}" = "cubic01" ]; then
    m=" -filter Cubic -define filter:b=0 -define filter:c=1 -define filter:blur=1 "
    w=" -filter cubic0.0,1.0 "
    f=" --down-filter cubic_0_1 --up-filter cubic_0_1"
  fi

  if [ "${i}" = "n_cubic" ]; then
    m=" -filter robidoux -define filter:blur=0.85574108326 "
    w=" -filter cubic0.37821575509399867,0.31089212245300067 -blur 0.85574108326"

  fi
    if [ "${i}" = "n_cubic_sharp" ]; then
    m=" -filter robidouxsharp -define filter:blur=0.90430390753 "
    w=" -filter cubic0.2620145123990142,0.3689927438004929 -blur 0.90430390753"
  fi
  echo 
  echo "====================================="
  echo "Testing ${i} using ${w} and ${m}"
  echo 
  #lanczos, mitchell, lanczos2, catrom,


  echo "../imagew ${w} -w 555 -h 275 ./pd.png  worsener_${i}_pd.png"
  ../imagew ${w} -nogamma -w 555 -h 275 ./pd.png  worsener_${i}_pd.png
  ../imagew ${w} -nogamma -w 555 -h 15 ./pl.png  worsener_${i}_pl.png
  ../imagew ${w} -nogamma -w 275 -h 555 ./pdr.png  worsener_${i}_pdr.png
  ../imagew ${w} -nogamma -w 15 -h 555 ./plr.png  worsener_${i}_plr.png

  echo "convert ./pd.png ${m} -resize 555x275\!  -colorspace sRGB magick_${i}_pd.png"
  convert ./pd.png ${m} -resize 555x275\!  -colorspace sRGB magick_${i}_pd.png
  convert ./pl.png ${m} -resize 555x15\!  -colorspace sRGB magick_${i}_pl.png
  convert ./pdr.png ${m} -resize 275x555\!  -colorspace sRGB magick_${i}_pdr.png
  convert ./plr.png ${m} -resize 15x555\!  -colorspace sRGB magick_${i}_plr.png

  echo "../imageflow_tool v1/querystring --quiet --in pd.png --out flow_${i}_pd.png --command=\"w=555&h=275&mode=stretch&scale=both&format=png&down.colorspace=srgb&up.colorspace=srgb&down.filter=${f}&up.filter=${f}\""
  ../imageflow_tool v1/querystring --quiet --in pd.png --out flow_${i}_pd.png --command="w=555&h=275&mode=stretch&scale=both&format=png&down.colorspace=srgb&up.colorspace=srgb&down.filter=${f}&up.filter=${f}"
  ../imageflow_tool v1/querystring --quiet --in pl.png --out flow_${i}_pl.png --command="w=555&h=15&mode=stretch&scale=both&format=png&down.colorspace=srgb&up.colorspace=srgb&down.filter=${f}&up.filter=${f}"
  ../imageflow_tool v1/querystring --quiet --in pdr.png --out flow_${i}_pdr.png --command="w=275&h=555&mode=stretch&scale=both&format=png&down.colorspace=srgb&up.colorspace=srgb&down.filter=${f}&up.filter=${f}"
  ../imageflow_tool v1/querystring --quiet --in plr.png --out flow_${i}_plr.png --command="w=15&h=555&mode=stretch&scale=both&format=png&down.colorspace=srgb&up.colorspace=srgb&down.filter=${f}&up.filter=${f}"

  echo "<h1>${i}</h1>" >> "./index.html"
  echo "<h4>Imageflow vs ImageWorsener</h4>" >> "./index.html"

  echo "<img src=\"scope_${i}_pd_h.png\" title=\"Downscaling dots horizontally\" />" >> "./index.html"
  echo "<img src=\"scope_${i}_pl_h.png\" title=\"Up-scaling line horizontally\" />" >> "./index.html"
  echo "<img src=\"scope_${i}_pdr_v.png\" title=\"Downscaling dots vertically\" />" >> "./index.html"
  echo "<img src=\"scope_${i}_plr_v.png\" title=\"Up-scaling line vertically\" />" >> "./index.html"

  ../rscope -thick -pd worsener_${i}_pd.png flow_${i}_pd.png scope_${i}_pd_h.png
  ../rscope -thick -pl worsener_${i}_pl.png flow_${i}_pl.png scope_${i}_pl_h.png
  ../rscope -thick -r -pd worsener_${i}_pdr.png  flow_${i}_pdr.png scope_${i}_pdr_v.png
  ../rscope -thick -r -pl worsener_${i}_plr.png  flow_${i}_plr.png scope_${i}_plr_v.png

#  echo "<h4>ImageMagick vs ImageWorsener</h4>" >> "./index.html"
#
#  echo "<img src=\"magick_scope_${i}_pd_h.png\" title=\"Downscaling dots horizontally\" />" >> "./index.html"
#  echo "<img src=\"magick_scope_${i}_pl_h.png\" title=\"Up-scaling line horizontally\" />" >> "./index.html"
#  echo "<img src=\"magick_scope_${i}_pdr_v.png\" title=\"Downscaling dots vertically\" />" >> "./index.html"
#  echo "<img src=\"magick_scope_${i}_plr_v.png\" title=\"Up-scaling line vertically\" />" >> "./index.html"


  ../rscope -thick -pd worsener_${i}_pd.png magick_${i}_pd.png magick_scope_${i}_pd_h.png
  ../rscope -thick -pl worsener_${i}_pl.png magick_${i}_pl.png magick_scope_${i}_pl_h.png
  ../rscope -thick -r -pd worsener_${i}_pdr.png magick_${i}_pdr.png  magick_scope_${i}_pdr_v.png
  ../rscope -thick -r -pl worsener_${i}_plr.png magick_${i}_plr.png  magick_scope_${i}_plr_v.png

  echo "<h4>Imageflow vs ImageMagick</h4>" >> "./index.html"

  echo "<img src=\"flow_magick_${i}_pd_h.png\" title=\"Downscaling dots horizontally\" />" >> "./index.html"
  echo "<img src=\"flow_magick_${i}_pl_h.png\" title=\"Up-scaling line horizontally\" />" >> "./index.html"
  echo "<img src=\"flow_magick_${i}_pdr_v.png\" title=\"Downscaling dots vertically\" />" >> "./index.html"
  echo "<img src=\"flow_magick_${i}_plr_v.png\" title=\"Up-scaling line vertically\" />" >> "./index.html"

  ../rscope -thick -pd magick_${i}_pd.png flow_${i}_pd.png  flow_magick_${i}_pd_h.png
  ../rscope -thick -pl magick_${i}_pl.png flow_${i}_pl.png  flow_magick_${i}_pl_h.png
  ../rscope -thick -r -pd magick_${i}_pdr.png flow_${i}_pdr.png  flow_magick_${i}_pdr_v.png
  ../rscope -thick -r -pl magick_${i}_plr.png flow_${i}_plr.png  flow_magick_${i}_plr_v.png
  
  rm ./{flow,magick,worsener}_${i}_{pd,pdr,pl,plr}.png
done

cd ..

firefox ./scope/index.html || open ./scope/index.html || x-www-browser ./scope/index.html
