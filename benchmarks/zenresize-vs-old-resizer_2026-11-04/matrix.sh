#!/usr/bin/env bash
set -u
X=$HOME/tmp/resize-why
cd $HOME/work/imageflow--resize-why
export CARGO_TARGET_DIR=$HOME/tmp/history-target
run() {
  label=$1; shift
  rm -f $X/manifests/$label.tsv
  env GIT_DIR=$HOME/work/imageflow/.git GIT_INDEX_FILE=$X/git-index GIT_OPTIONAL_LOCKS=0 \
    REGRESS_MANIFEST_PATH=$X/manifests/$label.tsv "$@" \
    nice -n19 ionice -c3 cargo nextest run -p imageflow_core --test integration --no-fail-fast > $X/$label.log 2>&1
  echo "$label: $(grep -E 'Summary' $X/$label.log | cut -c1-90)"
}
ALL="ZR_OLD_DECODE=1 ZR_OLD_ENCODE=1 ZR_OLD_UNPREMUL=1 ZR_OLD_WEIGHTS=1 ZR_OLD_KERNELS=1 ZR_OLD_F32CACHE=1"
run control2
run all ZENRESIZE_FORCE_F32=1 $ALL
for f in DECODE ENCODE UNPREMUL WEIGHTS KERNELS F32CACHE; do
  run only_$f ZENRESIZE_FORCE_F32=1 ZR_OLD_$f=1
  run without_$f ZENRESIZE_FORCE_F32=1 $(echo $ALL | tr ' ' '\n' | grep -v "ZR_OLD_$f=")
done
run all_no_force_f32 $ALL
