#!/usr/bin/env bash
# Downloads every input in inputs.tsv into <dir>.   fetch.sh <dir>
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
dir=$(mkdir -p "${1:?usage: fetch.sh <dir>}" && cd "$1" && pwd)
base=https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs
grep -v '^#' "$here/inputs.tsv" | while IFS=$'\t' read -r name path _; do
    ext=${path##*.}
    [ -s "$dir/$name.$ext" ] || curl -fsSL -o "$dir/$name.$ext" "$base/$path"
done
