#!/bin/bash

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

cd "$1" || exit

"$SCRIPT_DIR/mods/validate_droplet.sh"

ssh -oStrictHostKeyChecking=no "root@$(cat droplet.addr)" "/bin/bash"

