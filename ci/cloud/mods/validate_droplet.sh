#/bin/bash
set -e

"$( dirname "${BASH_SOURCE[0]}" )/validate_dir.sh" "$@"

if [[ -z "$(cat droplet.name)" ]]; then
	echo "droplet.name is missing! Wrong dir? $(pwd)"
	exit 3
fi 
if [[ -z "$(cat droplet.id)" ]]; then
	echo "droplet.id is missing! Wrong dir? $(pwd)"
	exit 3
fi 