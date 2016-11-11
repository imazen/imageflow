#!/bin/bash
set -e

./compare_reset_flow_images.sh
rm ./flow-proto1
./install_tools.sh
./compare.rb