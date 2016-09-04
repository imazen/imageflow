#!/bin/bash
set -e #Exit on failure.

rm ../target/debug/imageflow_lib_exercise || true
cargo build --bin imageflow_lib_exercise
valgrind ../target/debug/imageflow_lib_exercise
