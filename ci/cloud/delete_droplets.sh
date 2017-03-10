#!/bin/bash
set -e
#/tmp/cores/core.imageflow_serve.4807.imageflow-test-template-2.1483921692


SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"


for d in */ ; do
    ( 
    	cd "$d"
    	"$SCRIPT_DIR/mods/delete.sh" || echo "Nothing to delete in $d"
    )
done
