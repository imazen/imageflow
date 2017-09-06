#!/bin/bash
set -e #Exit on failure.

export CI_TAG="v0.1-prerelease0"
export PACKAGE_SUFFIX="x86_64-linux-gcc48-eglibc219"
export NUGET_RUNTIME="win9-x64"
echo "hello" > ./../../target/release/imageflow.dll
./pack.sh
