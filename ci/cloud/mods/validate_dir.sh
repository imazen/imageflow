#/bin/bash
set -e

has_shellcheck() {
	command -v shellcheck >/dev/null 2>&1 
}
if has_shellcheck; then
	shellcheck *.sh
	shellcheck ../*.sh
fi 

if [[ -f "droplet.name" ]]; then
	VAL="$(tr -d '[:space:]' < droplet.name)"
	printf "%s" "$VAL" > droplet.name

	if [[ -z "$(cat droplet.name)" ]]; then
		echo "$(pwd)/droplet.name is empty!"
		exit 3
	fi 
else
	echo "droplet.name must be defined in the provided directory!"
	exit 1;
fi 
