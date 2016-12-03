#!/bin/bash

## Remove dotfiles
find . -type f -name '*.dot' -exec rm {} +
find . -type f -name '*.dot.png' -exec rm {} +
## Remove frames
find -type d -name node_frames -exec rm -rf {} \;

## Remove frames
find -type d -name self_tests -exec rm -rf {} \;

# Remove cargo fmt tempfiles
find . -type f -name '*.rs.bk' -exec rm {} +

# Remove disassembly files in c_components
find . -type f -name '*.c.s' -exec rm {} +

