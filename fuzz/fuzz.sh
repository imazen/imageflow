#!/bin/bash
# Wrapper for cargo fuzz that instruments C decoder code for coverage.
#
# Without this, only Rust code has coverage instrumentation — the C decoders
# (mozjpeg, libpng, giflib, libwebp) are invisible to libfuzzer's coverage
# guidance. This sets CC=clang with sancov flags so all C code gets
# inline-8bit-counters, giving ~37K additional coverage points.
#
# Usage: ./fuzz.sh run fuzz_decode -- -fork=4 -dict=imageflow.dict -max_total_time=120

set -euo pipefail

export CC=clang
export CXX=clang++
export CFLAGS="-fsanitize-coverage=inline-8bit-counters,indirect-calls,trace-cmp,pc-table"
export CXXFLAGS="-fsanitize-coverage=inline-8bit-counters,indirect-calls,trace-cmp,pc-table"

exec cargo +nightly fuzz "$@"
