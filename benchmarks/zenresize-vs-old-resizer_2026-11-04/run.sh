#!/usr/bin/env bash
# Runs the integration visual suite on the harness branch twice: zenresize's own path choice, then forced f32.
set -u
X=$HOME/tmp/resize-why
cd $HOME/work/imageflow--resize-why
export CARGO_TARGET_DIR=$HOME/tmp/history-target
run() {
  label=$1; shift
  rm -f $X/manifests/$label.tsv
  $HOME/work/claudehints/scripts/run-heavy --mem 12G -- env GIT_DIR=$HOME/work/imageflow/.git GIT_INDEX_FILE=$X/git-index GIT_OPTIONAL_LOCKS=0 \
    REGRESS_MANIFEST_PATH=$X/manifests/$label.tsv "$@" \
    cargo nextest run -p imageflow_core --test integration --no-fail-fast > $X/$label.log 2>&1
  grep -E 'Summary|run-heavy: done|^error' $X/$label.log | sed "s/^/$label: /"
}
rm -f $X/paths.log
run control ZENRESIZE_PATH_LOG=$X/paths.log
run f32 ZENRESIZE_FORCE_F32=1
