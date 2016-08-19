#!/bin/bash

set -e #Exit on failure.
set -x

sudo . /usr/local/lib/rustlib/uninstall.sh | true

sudo rm -rf /rust

export RUST_ARCHIVE=rust-nightly-x86_64-unknown-linux-gnu.tar.gz
export RUST_DOWNLOAD_URL=https://static.rust-lang.org/dist/$RUST_ARCHIVE

mkdir -p ~/rust
cd ~/rust

curl -fsOSL $RUST_DOWNLOAD_URL \
    && curl -s $RUST_DOWNLOAD_URL.sha256 | sha256sum -c - \
    && tar -C ~/rust -xzf $RUST_ARCHIVE --strip-components=1 \
    && rm $RUST_ARCHIVE \
    && ./install.sh
