#!/bin/bash
set -e

"$( dirname "${BASH_SOURCE[0]}" )/validate_droplet.sh" "$@"


ssh -oStrictHostKeyChecking=no "root@$(cat droplet.addr)" /bin/bash <<EOF1
	curl -sSL https://agent.digitalocean.com/install.sh | sh
EOF1
