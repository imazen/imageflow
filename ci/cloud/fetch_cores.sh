#!/bin/bash
set -e
#/tmp/cores/core.imageflow_serve.4807.imageflow-test-template-2.1483921692


SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

cd "$1"

"$SCRIPT_DIR/mods/validate_droplet.sh"


mkdir "$SCRIPT_DIR/cores"
scp  -oStrictHostKeyChecking=no "root@$(cat droplet.addr):/tmp/cores/*" "$SCRIPT_DIR/cores/"
