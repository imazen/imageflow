#!/bin/bash
set -e
rm ./imageflow_tool
./compare_reset_flow_images.sh
./install_tools.sh
./compare.rb