#!/usr/bin/env bash
# Decodes every input in inputs.tsv to PNG with two builds of dump.rs (one per moxcms version),
# once with moxcms and once with lcms2, then compares them with pxcmp.py.
#
#   run.sh <old-dump-binary> <new-dump-binary> <inputs-dir> <out-dir>
#
# Build each binary by copying dump.rs to imageflow_core/examples/moxcms_dump.rs and running
# `cargo build -p imageflow_core --example moxcms_dump --profile test --locked` on each version.
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
old=${1:?usage: run.sh <old> <new> <inputs-dir> <out-dir>} new=${2:?} in=${3:?}
out=$(mkdir -p "${4:?}" && cd "$4" && pwd)
names=
while IFS=$'\t' read -r name path _; do
    ext=${path##*.}
    for v in old new; do
        bin=$old; [ $v = new ] && bin=$new
        for cms in moxcms lcms2; do
            "$bin" $cms "$in/$name.$ext" "$out/${v}_${cms}_$name.png"
        done
    done
    names="$names $name"
done < <(grep -v '^#' "$here/inputs.tsv")
python3 "$here/pxcmp.py" "$out" $names
