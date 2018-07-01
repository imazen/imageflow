#!/bin/bash

set -e #Exit on failure.
set -x

curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain beta
