#/bin/bash
set -e

"$( dirname "${BASH_SOURCE[0]}" )/validate_droplet.sh" "$@"


if [[ -z "$1" ]]; then
	echo "You must provide a snapshot ID as the first argument!"
	exit 3
fi 

echo Powering off
./doctl compute droplet-action power-off "$(cat droplet.id)" --wait
./doctl compute droplet-action snapshot "$(cat droplet.id)" --snapshot-name "$1" --wait -o json | tee "$1.json"
echo "Snapshot created (still off)"
doctl compute  image list-user

echo "Delete with: ./doctl compute  image delete $2"
