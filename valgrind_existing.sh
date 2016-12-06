#!/bin/bash
set -e #Exit on failure.

export VALGRIND_COMMAND="valgrind -q --error-exitcode=9 --gen-suppressions=all"
export RUST_BACKTRACE=1

lastmod(){
     echo "Modified" $(( $(date +%s) - $(stat -c%Y "$1") )) "seconds ago"
}

STAMP="+[%H:%M:%S]"
date "$STAMP"

echo "Running all test executables in target/debug under Valgrind"
TEST_BINARIES="$(ls ./target/debug/*-[a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9][a-f0-9])"
printf "%s Discovered binaries:\n" "$(date '+[%H:%M:%S]')"
echo "${TEST_BINARIES[@]}"

# The flow_proto1 test suite kills valgrind
SKIP_BINARIES=("$(ls ./target/debug/flow_proto1* || true )")
SKIP_BINARIES+=("$(ls ./target/debug/flow_proto1* || true )")
echo "Should skip: ${SKIP_BINARIES[@]}"

for f in $TEST_BINARIES
do
	printf "\n==============================================================\n%s %s\n" "$(date '+[%H:%M:%S]')" "$f"
	if [[ " ${SKIP_BINARIES[@]} " =~ " ${f} " ]]; then
		echo "SKIPPING"
	else
	  lastmod "$f"
	  
	  FULL_COMMAND="$VALGRIND_COMMAND $f"
	  printf "\n%s\n" "$FULL_COMMAND"
	  export VALGRIND_RUNNING=1
	  eval "$FULL_COMMAND"
	  
	fi
done
date "$STAMP"