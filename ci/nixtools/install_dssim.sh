#!/bin/bash

set -e #Exit on failure.
set -x

if [[ "$(uname -s)" != 'Darwin' ]]; then
  sudo apt-get install -y -q pkg-config libpng-dev
fi

wget -O dssim.tar.gz https://github.com/pornel/dssim/archive/c6ad29c5a2dc37d8610120486f09eda145621c84.tar.gz && \
tar xvzf dssim.tar.gz && mv dssim-c6ad29c5a2dc37d8610120486f09eda145621c84 dssim && cd dssim && \
make && sudo cp bin/dssim /usr/local/bin/dssim && cd .. && rm -rf dssim 
