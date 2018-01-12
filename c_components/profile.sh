#!/bin/bash
set -e

(
    cd tests;
    cargo build --bin profile_imageflow --release
)

BIN=../target/release/profile_imageflow

time "$BIN"
gprof "$BIN" gmon.out > profile.txt
