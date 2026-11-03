#!/usr/bin/env bash
# Builds moxcms-stack-probe against moxcms 0.8.1, 0.9.1 and upstream master, then finds the
# smallest thread stack on which each transform builds, and compares 8-bit High and Low output.
#
#   benchmarks/issue-737-iis-repro_2026-11-03/moxcms-stack-probe/run.sh <out-dir>
#
# Writes <out-dir>/sweep.tsv (every run), <out-dir>/summary.txt, and downloads the two test JPEGs
# into <out-dir>. The three release builds use fat LTO.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../.." && pwd)
out=$(mkdir -p "${1:?usage: run.sh <out-dir>}" && cd "$1" && pwd)
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HOME/tmp/moxcms-stack-probe-target}

base=https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs
for name in MarsRGB_v4_sYCC_8bit.jpg MarsRGB_tagged.jpg; do
    [ -s "$out/$name" ] || curl -fsSL -o "$out/$name" "$base/$name"
done
# name<TAB>path: imageflow's built-in CMYK profile (what CMYK JPEGs use), an RGB JPEG whose
# profile is LUT-only, and an RGB JPEG with a matrix/TRC profile (control: no LUT is built).
inputs=$(printf '%s\t%s\n' \
    cmyk "$repo/imageflow_core/src/codecs/cmyk.icc" \
    rgb-lut "$out/MarsRGB_v4_sYCC_8bit.jpg" \
    rgb-matrix "$out/MarsRGB_tagged.jpg")

versions="v0_8_1 v0_9_1 master"
for v in $versions; do
    cargo build --release --locked --manifest-path "$here/Cargo.toml" --features "$v"
    cp "$CARGO_TARGET_DIR/release/moxcms-stack-probe" "$out/probe-$v"
done

sizes="64 128 192 256 320 384 448 512 576 640 704 736 768 800 832 864 896 960 1024 1536 2048 4096"
{
    echo "# $(date -u +%Y-%m-%dT%H:%M:%SZ) cpu=$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | xargs) $(uname -srm) $(rustc --version)"
    echo "# imageflow $(git -C "$repo" rev-parse --short=10 HEAD 2>/dev/null || echo unknown)"
    printf 'version\tinput\tscale\tbits\tstack_kib\tresult\n'
} > "$out/sweep.tsv"
: > "$out/summary.txt"

while IFS=$'\t' read -r name path; do
    for v in $versions; do
        "$out/probe-$v" info "$path" | sed "s/^/$name: /" >> "$out/summary.txt"
    done
done <<< "$inputs"

for v in $versions; do
    while IFS=$'\t' read -r name path; do
        for mode in "high 8" "low 8" "high 16"; do
            set -- $mode
            max_fail=none min_ok=none
            for kib in $sizes; do
                set +e
                "$out/probe-$v" build "$path" "$kib" "$1" "$2" > "$out/last.out" 2> "$out/last.err"
                rc=$?
                set -e
                if [ $rc -eq 0 ]; then
                    result=ok
                    [ "$min_ok" = none ] && min_ok=$kib
                elif grep -q "overflowed its stack" "$out/last.err"; then
                    result=overflow
                    max_fail=$kib
                else
                    result="rc$rc:$(cat "$out/last.out" "$out/last.err" | tr '\n\t' '  ' | cut -c1-80)"
                    max_fail=$kib
                fi
                printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$v" "$name" "$1" "$2" "$kib" "$result" >> "$out/sweep.tsv"
            done
            printf '%-7s %-10s %-4s %2s-bit: largest failing stack %s KiB, smallest passing %s KiB\n' \
                "$v" "$name" "$1" "$2" "$max_fail" "$min_ok" >> "$out/summary.txt"
        done
    done <<< "$inputs"
done
rm -f "$out/last.out" "$out/last.err"

for v in $versions; do
    while IFS=$'\t' read -r name path; do
        [ "$name" = rgb-matrix ] && continue
        "$out/probe-$v" diff "$path" | sed "s/^/$name: /" >> "$out/summary.txt"
    done <<< "$inputs"
done
cat "$out/summary.txt"
