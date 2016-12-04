#!/bin/bash
set -e #Exit on failure.


OUT_FILE="imageflow_ffi.lua"

printf "local ffi = require(\"ffi\")\nffi.cdef[[\n" > "$OUT_FILE"
cat "../headers/imageflow_lua.h" >> "$OUT_FILE"
printf "\n]]\nreturn ffi.C" >> "$OUT_FILE"
