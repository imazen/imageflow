#!/bin/bash
set -e #Exit on failure.

#Underscores are prohibited in prerelease tags
export CI_TAG="v0.9-rc1-1"
export PACKAGE_SUFFIX="win-x64"
export NUGET_RUNTIME="win-x64"
export BINARIES_DIR="target/release/"
export REPO_NAME="imazen\/imageflow"

# Save current directory
SAVE_DIR=$(pwd)


# cd to root of repo, or fallback to current script plus ../..
cd $(git rev-parse --show-toplevel) || cd $(dirname $0)/../..

# if BINARIES_DIR doesn't exist, relative to root of repo, run cargo build --release 
if [ ! -d "$BINARIES_DIR" ]; then
    cargo build --release || cd $SAVE_DIR
fi

cd ci/pack_nuget

./pack.sh || cd $SAVE_DIR

cd $SAVE_DIR
