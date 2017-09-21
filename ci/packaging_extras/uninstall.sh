#!/bin/bash
set -e #Exit on failure.

# Set INSTALL_BASE to customize install/uninstall location
export INSTALL_BASE="${INSTALL_BASE:-/usr/local}"

echo "Removing libimageflow, imageflow_tool, imageflow_server binaries (you may need sudo)"
rm "${INSTALL_BASE}/lib/libimageflow.so" || true
rm "${INSTALL_BASE}/lib/libimageflow.dylib" || true
rm "${INSTALL_BASE}/include/imageflow.h" || true
rm "${INSTALL_BASE}/bin/imageflow_tool" || true
rm "${INSTALL_BASE}/bin/imageflow_server" || true
