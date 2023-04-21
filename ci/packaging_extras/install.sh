#!/bin/bash
set -e #Exit on failure.

# Change directory to root (call this in a subshell if you have a problem with that)
cd "$( dirname "${BASH_SOURCE[0]}" )"


if [[ "$(uname -s)" == 'Darwin' ]]; then
	export DLL_EXT="dylib"
else
	export DLL_EXT="so"
fi

# Set INSTALL_BASE to customize install location
export INSTALL_BASE="${INSTALL_BASE:-/usr/local}"

if [[ ! -e "./libimageflow.${DLL_EXT}" || ! -e "./imageflow_tool" ]]; then
    echo Cannot install - libimageflow or imageflow_tool not found
    exit 1;
fi
cp "./libimageflow.${DLL_EXT}" "${INSTALL_BASE}/lib/"
cp "./imageflow_tool" "${INSTALL_BASE}/bin/"
cp "./imageflow.h" "${INSTALL_BASE}/include/imageflow.h"
echo "Installed libimageflow and imageflow_tool"
exit 0;
