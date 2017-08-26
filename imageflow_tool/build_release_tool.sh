#!/bin/bash

RUSTFLAGS="-C target-cpu=native" CARGO_INCREMENTAL=1  cargo build --release
cp ../target/release/imageflow_tool .
./imageflow_tool --version