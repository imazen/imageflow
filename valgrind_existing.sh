#!/bin/bash
set -e #Exit on failure.

VALGRIND_ARGS="-q --error-exitcode=9 --gen-suppressions=all"

# Valgrind script args, or fallback to discovering them in ./target/debug
TEST_BINARIES=("$@")

printf "%s valgrind_existing.sh " "$(date '+[%H:%M:%S]')"

if [ "$#" -lt 1 ]; then
	shopt -s nullglob
	TEST_BINARIES=(./target/debug/*-[a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9])
	shopt -u nullglob
	printf "discovered binaries:\n"
else
	printf "passed arguments:\n"
fi
printf "%s\n" "${TEST_BINARIES[@]}"
#echo "${TEST_BINARIES[@]}"

# Sometimes we may need to exclude binaries
SKIP_BINARIES=()
#SKIP_BINARIES+=("$(ls ./target/debug/flow_proto1* || true )")
echo "Should skip: ${SKIP_BINARIES[@]}"


# If we're running as 'conan' (we assume this indicates we are in a docker container)
# Then we need to also change permissions so that .valgrindrc is respected
# It cannot be world-writable, and should be owned by the current user (according to valgrind)
export CHOWN_VALGRIND_FILE_IF_USER_IS="${CHOWN_VALGRIND_FILE_IF_USER_IS:-conan}"

create_valgrind_files_in()(
	(
		SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

		cd "$1"
		FILE_NAMES=(".valgrindrc" "valgrind_suppressions.txt")
		for f in "${FILE_NAMES[@]}"
		do
			cp "${SCRIPT_DIR}/${f}" "./${f}"
			if [[ "$(id -u -n)" == "${CHOWN_VALGRIND_FILE_IF_USER_IS}" ]]; then
				sudo chown "${CHOWN_VALGRIND_FILE_IF_USER_IS}:" "./${f}"
				sudo chmod o-w "./${f}"
			fi
		done	
	)
)


print_modified_ago(){
     echo "Modified" $(( $(date +%s) - $(stat -c%Y "$1") )) "seconds ago"
}

for f in "${TEST_BINARIES[@]}"
do
	printf "\n==============================================================\n%s %s\n" "$(date '+[%H:%M:%S]')" "$f"
	if [[ " ${SKIP_BINARIES[@]} " == *" ${f} "* ]]; then
		echo "SKIPPING"
	else
	  print_modified_ago "$f"
	  

	  REL_F=$(basename "${f}")
	  DIR=$(dirname "${f}")
	  DIR="${DIR}/grind_${REL_F}"
	  mkdir -p "${DIR}" || true

	  create_valgrind_files_in "$DIR"

	  FULL_COMMAND="(cd $DIR && valgrind $VALGRIND_ARGS ../$REL_F)"
	  printf "\n%s\n" "$FULL_COMMAND"

	  export VALGRIND_RUNNING=1
	  export RUST_BACKTRACE=1
	  eval "$FULL_COMMAND"
	  
	fi
done

printf "\n%s Completed valgrind_existing.sh (" "$(date '+[%H:%M:%S]')"
printf "%s " "${TEST_BINARIES[@]}"
printf ")\n"