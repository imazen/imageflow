#!/bin/bash


cd ../..
conan remove imageflow/* -f
conan export lasote/testing

cd wrappers/server

conan install --build missing # Will build imageflow package with your current settings
cargo build
