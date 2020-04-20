#!/bin/bash
set -e

"$( dirname "${BASH_SOURCE[0]}" )/validate_dir.sh" "$@"

printf "\n\n===== Deleting droplet %s and %s ======\n" "$(cat droplet.name)" "$(cat droplet.id)"

if [[ -f "droplet.id" ]]; then 
	doctl compute droplet delete "$(cat droplet.id)" --force || true
fi 

doctl compute droplet delete "$(cat droplet.name)" --force || true

rm ./droplet.{id,json,addr} || true

printf "\n===== Remaining droplets ======\n" 

echo "doctl compute droplet list"
doctl compute droplet list