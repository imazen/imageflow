#!/bin/bash

set -e #Exit on failure.
set -x

curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly-2016-09-01

rustup target add x86_64-unknown-linux-musl 