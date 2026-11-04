#!/usr/bin/env bash
# Builds moxcms-stack-probe against each moxcms version in $VERSIONS (default: upstream master
# and the patched fork), in release (fat LTO) and debug, then finds the smallest thread stack on
# which each transform builds and runs, and checks that every version's output matches.
#
#   benchmarks/issue-737-iis-repro_2026-11-03/moxcms-stack-probe/run.sh <out-dir>
#
# VERSIONS may also name v0_8_1 and v0_9_1. Writes <out-dir>/sweep.tsv (one row per transform:
# every stack size tried, in KiB, and its result), <out-dir>/summary.txt, and downloads the two
# test JPEGs into <out-dir>.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../.." && pwd)
out=$(mkdir -p "${1:?usage: run.sh <out-dir>}" && cd "$1" && pwd)
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HOME/tmp/moxcms-stack-probe-target}
versions=${VERSIONS:-master patched}

base=https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs
for name in MarsRGB_v4_sYCC_8bit.jpg MarsRGB_tagged.jpg; do
    [ -s "$out/$name" ] || curl -fsSL -o "$out/$name" "$base/$name"
done
# name<TAB>profile: imageflow's built-in CMYK profile (what CMYK JPEGs use), an RGB JPEG whose
# profile is LUT-only, an RGB JPEG with a matrix/TRC profile, and two built-in profiles.
inputs=$(printf '%s\t%s\n' \
    cmyk "$repo/imageflow_core/src/codecs/cmyk.icc" \
    rgb-lut "$out/MarsRGB_v4_sYCC_8bit.jpg" \
    rgb-matrix "$out/MarsRGB_tagged.jpg" \
    display-p3 builtin:display-p3 \
    gray-2.2 builtin:gray-2.2)
modes="high:8 low:8 high:10 high:12 high:16 high:f32 high:f64"

binaries=
for v in $versions; do
    cargo build --release --locked --manifest-path "$here/Cargo.toml" --features "$v"
    cp "$CARGO_TARGET_DIR/release/moxcms-stack-probe" "$out/probe-$v-release"
    cargo build --locked --manifest-path "$here/Cargo.toml" --features "$v"
    cp "$CARGO_TARGET_DIR/debug/moxcms-stack-probe" "$out/probe-$v-debug"
    binaries="$binaries $v-release $v-debug"
done

header="# $(date -u +%Y-%m-%dT%H:%M:%SZ) cpu=$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | xargs) $(uname -srm) $(rustc --version)"
header="$header"$'\n'"# imageflow $(git -C "$repo" rev-parse --short=10 HEAD 2>/dev/null || echo unknown)"
{
    echo "$header"
    printf 'build\tinput\tscale\tdepth\tmin_ok_kib\ttried\n'
} > "$out/sweep.tsv"

# Prints ok, overflow or the error text of one run.
try() {
    local bin=$1 profile=$2 kib=$3 scale=$4 depth=$5 rc
    set +e
    "$out/probe-$bin" build "$profile" "$kib" "$scale" "$depth" run > "$out/last.out" 2> "$out/last.err"
    rc=$?
    set -e
    if [ $rc -eq 0 ]; then
        echo ok
    elif grep -q "overflowed its stack\|stack overflow" "$out/last.err"; then
        echo overflow
    else
        echo "rc$rc:$(cat "$out/last.out" "$out/last.err" | tr '\n\t' '  ' | cut -c1-80)"
    fi
}

# Binary search, in 16 KiB steps, for the smallest stack that works (at most 8192 KiB).
for bin in $binaries; do
    while IFS=$'\t' read -r name profile; do
        for mode in $modes; do
            scale=${mode%%:*} depth=${mode#*:}
            lo=0 hi=8192
            r=$(try "$bin" "$profile" $hi "$scale" "$depth")
            tried="$hi:$r"
            if [ "$r" != ok ]; then
                min_ok="none($r)"
            else
                while [ $((hi - lo)) -gt 16 ]; do
                    mid=$(( (lo + hi) / 32 * 16 ))
                    [ $mid -le $lo ] && mid=$((lo + 16))
                    r=$(try "$bin" "$profile" $mid "$scale" "$depth")
                    tried="$tried $mid:$r"
                    case $r in
                        ok) hi=$mid ;;
                        overflow) lo=$mid ;;
                        *) echo "unexpected result at $mid KiB: $r" >&2; exit 1 ;;
                    esac
                done
                min_ok=$hi
            fi
            printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$bin" "$name" "$scale" "$depth" "$min_ok" "$tried" >> "$out/sweep.tsv"
        done
    done <<< "$inputs"
done
rm -f "$out/last.out" "$out/last.err"

{
    echo "$header"
    echo "# Smallest thread stack (KiB, 16 KiB steps) on which the transform to sRGB builds and"
    echo "# transforms 4096 pixels; 16 means 16 or less."
    printf '%-10s %-5s %-5s' input scale depth
    for bin in $binaries; do printf ' %15s' "$bin"; done
    echo
    while IFS=$'\t' read -r name profile; do
        for mode in $modes; do
            scale=${mode%%:*} depth=${mode#*:}
            printf '%-10s %-5s %-5s' "$name" "$scale" "$depth"
            for bin in $binaries; do
                printf ' %15s' "$(awk -F'\t' -v b="$bin" -v n="$name" -v s="$scale" -v d="$depth" \
                    '$1==b && $2==n && $3==s && $4==d {print $5}' "$out/sweep.tsv")"
            done
            echo
        done
    done <<< "$inputs"
    echo
    echo "# Output digests (release builds); every version must print the same hashes."
    first=${versions%% *}
    while IFS=$'\t' read -r name profile; do
        for depth in 8 10 12 16 f32 f64; do
            for v in $versions; do
                "$out/probe-$v-release" digest "$profile" "$depth" |
                    sed -E "s/^moxcms [^:]+: /$name /" > "$out/digest-$v.txt"
            done
            for v in $versions; do
                if cmp -s "$out/digest-$first.txt" "$out/digest-$v.txt"; then same=same; else same=DIFFERENT; fi
                sed "s/\$/ [$v: $same]/" "$out/digest-$v.txt"
            done
        done
    done <<< "$inputs"
    rm -f "$out"/digest-*.txt
    echo
    while IFS=$'\t' read -r name profile; do
        for v in $versions; do
            "$out/probe-$v-release" info "$profile" | sed "s/^/$name: /"
        done
    done <<< "$inputs"
} > "$out/summary.txt"
cat "$out/summary.txt"
