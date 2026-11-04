#!/usr/bin/env bash
# Builds a moxcms checkout as a library for several targets and profiles and lists every
# function whose prologue reserves at least <min_bytes> of stack (default 16384).
#
#   moxcms-stack-probe/frames.sh <moxcms-checkout> <out.tsv> [min_bytes]
#   REV=097dccf3a9 moxcms-stack-probe/frames.sh <exported-tree> <out.tsv> [min_bytes]
#
# Library builds need no linker, so every target only needs `rustup target add`. aarch64
# objects are read with aarch64-linux-gnu-objdump (binutils-aarch64-linux-gnu).
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
src=$(cd "${1:?usage: frames.sh <moxcms-checkout> <out.tsv> [min_bytes]}" && pwd)
out=${2:?usage: frames.sh <moxcms-checkout> <out.tsv> [min_bytes]}
min=${3:-16384}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HOME/tmp/moxcms-frames-target}

# imageflow's features, and every feature that builds on stable (sve needs nightly).
imageflow=in_place,options
all=avx512,avx512_shaper_optimized_paths,avx512_shaper_fixed_point_paths,avx_shaper_optimized_paths
all=$all,sse_shaper_optimized_paths,neon_shaper_optimized_paths,options,any_to_any,extended_range,in_place

configs="
x86_64-unknown-linux-gnu release imageflow
x86_64-unknown-linux-gnu release all
x86_64-unknown-linux-gnu dev all
x86_64-pc-windows-msvc release all
i686-pc-windows-msvc release all
i686-unknown-linux-gnu release all
aarch64-unknown-linux-gnu release all
"

# REV names a source tree without version control (for example a `git archive` export).
if [ -n "${REV:-}" ]; then
    rev=$REV
elif [ -d "$src/.jj" ]; then
    rev=$(jj --repository "$src" log -r @ --no-graph -T 'commit_id.short(10)')
else
    rev=$(git -C "$src" rev-parse --short=10 HEAD)
fi
{
    echo "# $(date -u +%Y-%m-%dT%H:%M:%SZ) moxcms $rev $(rustc --version) min_bytes=$min"
    printf 'target\tprofile\tfeatures\tbytes\tfunction\n'
} > "$out"

while read -r target profile fset; do
    [ -n "$target" ] || continue
    features=${!fset}
    if [ "$profile" = release ]; then dir=release; flag=--release; else dir=debug; flag=; fi
    cargo build --quiet --manifest-path "$src/Cargo.toml" --lib --target "$target" $flag \
        --features "$features"
    case $target in
        aarch64-*) objdump=aarch64-linux-gnu-objdump ;;
        *) objdump=objdump ;;
    esac
    python3 -I "$here/frames.py" "$objdump" "$CARGO_TARGET_DIR/$target/$dir/libmoxcms.rlib" "$min" |
        while IFS=$'\t' read -r bytes fn; do
            printf '%s\t%s\t%s\t%s\t%s\n' "$target" "$profile" "$fset" "$bytes" "$fn"
        done >> "$out"
done <<< "$configs"
cut -f1-3 "$out" | grep -v '^#' | sort | uniq -c
