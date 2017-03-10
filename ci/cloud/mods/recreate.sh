#!/bin/bash
set -e

"$( dirname "${BASH_SOURCE[0]}" )/delete.sh" "$@"

printf "\n\n===== Creating droplet %s ======\n" "$(cat droplet.name)"

doctl compute droplet create "$(cat droplet.name)" "$@" --wait -o json | tee "droplet.json"

jq '.[0].id' < droplet.json > "droplet.id"

DROPLET_ADDR="$(jq '.[0].networks.v4[0].ip_address' < droplet.json)"
DROPLET_ADDR="${DROPLET_ADDR%\"}"
DROPLET_ADDR="${DROPLET_ADDR#\"}"
printf "%s" "$DROPLET_ADDR" > "droplet.addr"
printf "\nDROPLET_ADDR=%s DROPLET_ID=%s\n" "$(cat droplet.addr)" "$(cat droplet.addr)" 


#Resumable past this point
sleep 15
