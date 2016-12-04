#!/bin/bash
set -e #Exit on failure.

(
  cd ../../imageflow_abi
  cargo build --lib
)

function dll_dir {
  (cd "../../target/debug" &>/dev/null && printf "%s/%s" "$PWD" "${1##*/}")
}

export RUST_BACKTRACE=1
export DYLIB_DIR="$(dll_dir)"

ls ${DYLIB_DIR}*.so

if [[ "$(uname -s)" == 'Darwin' ]]; then
  export DYLD_LIBRARY_PATH="${DYLIB_DIR}:${DYLD_LIBRARY_PATH}"
else
  export LD_LIBRARY_PATH="${DYLIB_DIR}:${LD_LIBRARY_PATH}"
fi

valgrind luajit "imageflow.lua"

