#!/bin/bash
set -e

"$( dirname "${BASH_SOURCE[0]}" )/validate_droplet.sh" "$@"


firefox "http://$(cat droplet.addr):3000/proxied_demo/index.html"

