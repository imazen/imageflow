#!/bin/bash

set -e 
set -x

cd ..
conan remove imageflow/* -f
conan export lasote/testing

cd imageflow_cdylib

conan install --build missing # Will build imageflow package with your current settings
cargo build --release
cargo doc --no-deps
