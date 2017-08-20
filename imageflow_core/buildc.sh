#!/bin/bash

set -e
set -x

cd ..

cd c_components
conan remove imageflow_c/* -f
conan export imazen/testing

cd ../imageflow_core

conan install --build missing -s target_cpu=haswell # Will build imageflow package with your current settings

CARGO_INCREMENTAL=1 cargo test
