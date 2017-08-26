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

../rscope -gen
../rscope -r -gen

rm ./rscope*.htm*


declare -a arr=("cubic01" "ncubic" "ncubicsharp" "lanczos" "lanczos2" "ginseng" "robidoux" "robidouxsharp" "triangle" "bspline" "hermite" "catrom" "mitchell")

## now loop through the above array
for i in "${arr[@]}"
do

  m="-filter ${i}"
  w="-filter ${i}"
  f="--down-filter ${i} --up-filter ${i}"
  if [ "${i}" = "ginseng" ]; then
    m=" -define filter:filter=Sinc -define filter:window=Jinc -define filter:lobes=3 "

  fi
  if [ "${i}" = "robidoux" ]; then
    w=" -filter cubic0.37821575509399867,0.31089212245300067 "
  fi
  if [ "${i}" = "robidouxsharp" ]; then
    w=" -filter cubic0.2620145123990142,0.3689927438004929 "
  fi
  if [ "${i}" = "bspline" ]; then
    w=" -filter bspline "
    m=" -filter spline "
  fi

  if [ "${i}" = "cubic01" ]; then
    m=" -filter Cubic -define filter:b=0 -define filter:c=1 -define filter:blur=1 "
    w=" -filter cubic0.0,1.0 "
    f=" --down-filter cubic_0_1 --up-filter cubic_0_1"
  fi

  if [ "${i}" = "ncubic" ]; then
    m=" -filter robidoux -define filter:blur=0.85574108326 "
    w=" -filter cubic0.37821575509399867,0.31089212245300067 -blur 0.85574108326"

    f=" --down-filter ncubic --up-filter ncubic"
  fi
    if [ "${i}" = "ncubicsharp" ]; then
    m=" -filter robidouxsharp -define filter:blur=0.90430390753 "
    w=" -filter cubic0.2620145123990142,0.3689927438004929 -blur 0.90430390753"
    f=" --down-filter ncubicsharp --up-filter ncubicsharp "
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
  
  echo "../flow-proto1 -i pd.png -o flow_${i}_pd.png -w 555 -h 275 -m 100 --constrain distort --format png24 --incorrectgamma ${f}"
  ../flow-proto1 -i pd.png -o flow_${i}_pd.png -w 555 -h 275 -m 100 --constrain distort --format png24 --incorrectgamma ${f}
  ../flow-proto1 -i pl.png -o flow_${i}_pl.png -w 555 -h 275 -m 100 --constrain distort --format png24 --incorrectgamma ${f}
  ../flow-proto1 -i pdr.png -o flow_${i}_pdr.png -w 275 -h 555 -m 100 --constrain distort --format png24 --incorrectgamma ${f}
  ../flow-proto1 -i plr.png -o flow_${i}_plr.png -w 15 -h 555 -m 100 --constrain distort --format png24 --incorrectgamma ${f}

  ../rscope -thick -pd worsener_${i}_pd.png flow_${i}_pd.png scope_${i}_pd_h.png
  ../rscope -thick -pl worsener_${i}_pl.png flow_${i}_pl.png scope_${i}_pl_h.png
  ../rscope -thick -r -pd worsener_${i}_pdr.png  flow_${i}_pdr.png scope_${i}_pd_v.png
  ../rscope -thick -r -pl worsener_${i}_plr.png  flow_${i}_plr.png scope_${i}_plr_v.png
  
  ../rscope -thick -pd worsener_${i}_pd.png magick_${i}_pd.png magick_scope_${i}_pd_h.png
  ../rscope -thick -pl worsener_${i}_pl.png magick_${i}_pl.png magick_scope_${i}_pl_h.png
  ../rscope -thick -r -pd worsener_${i}_pdr.png magick_${i}_pdr.png  magick_scope_${i}_pd_v.png
  ../rscope -thick -r -pl worsener_${i}_plr.png magick_${i}_plr.png  magick_scope_${i}_plr_v.png
  
  ../rscope -thick -pd magick_${i}_pd.png flow_${i}_pd.png  flow_magick_${i}_pd_h.png
  ../rscope -thick -pl magick_${i}_pl.png flow_${i}_pl.png  flow_magick_${i}_pl_h.png
  ../rscope -thick -r -pd magick_${i}_pdr.png flow_${i}_pdr.png  flow_magick_${i}_pd_v.png
  ../rscope -thick -r -pl magick_${i}_plr.png flow_${i}_plr.png  flow_magick_${i}_plr_v.png
  
  rm ./{flow,magick,worsener}_${i}_{pd,pdr,pl,plr}.png
done

cd ..

firefox ./scope || open ./scope
