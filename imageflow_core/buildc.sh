#!/bin/bash

set -e 
set -x

cd ..
conan remove imazen/* -f
conan export imazen/testing

cd imageflow_core

conan install --build missing # Will build imageflow package with your current settings
cargo build
