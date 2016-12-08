#!/bin/bash
set -e

echo THIS DOES NOT RUN TESTS

if [[ "$1" == 'debug' ]]; then
	DEBUG=1
else
	DEBUG=0
fi

if [[ "$2" == 'tiny' ]]; then
	TINY=1
else
	TINY=0
fi



print_modified_ago(){
     echo "Modified" $(( $(date +%s) - $(stat -c%Y "$1") )) "seconds ago"
}
(
	if [[ "$DEBUG" == '1' ]]; then
		cargo build --bin imageflow_server
	else
		cargo build --release --bin imageflow_server
	fi
)
(
	cd ../ci/docker/imageflow_server

	if [[ "$DEBUG" == '1' ]]; then
		BINARY_DIR="../../../target/debug"
	
	else
		BINARY_DIR="../../../target/release"
	fi

	mkdir bin || true
	set -x
	cp -p "${BINARY_DIR}/imageflow_server" "./bin/"
	set +x
	print_modified_ago "./bin/imageflow_server"

	if [[ "$TINY" == '1' ]]; then
	    IMAGE_NAME=imageflow_server_tiny
	    (
	        cd tiny
	        mkdir bin || true
	        cp -p ../bin/imageflow_server ./bin/imageflow_server
	        # statifier doesn't work
	        # statifier ./bin/imageflow_server
            DEPS=($(ldd ./bin/imageflow_server | grep -o "/[^ ]*"))
            cp -p Dockerfile Dockerfile.extra
	        for i in "${DEPS[@]}"
            do
              cp -p "$i" "./bin/"
              BASENAME=$(basename "$i")
              printf "\nCOPY ./bin/%s ./root/%s\n" "${BASENAME}" "${BASENAME}" >> Dockerfile.extra
            done

	        docker build -t "imazen/${IMAGE_NAME}" "$(pwd)" -f Dockerfile.extra
	        docker run --rm "imazen/${IMAGE_NAME}"  /bin/sh -c "LD_LIBRARY_PATH=/root && /root/imageflow_server --version"

            docker push  imazen/${IMAGE_NAME}

        #docker run --rm "imazen/${IMAGE_NAME}" sudo "/home/conan/imageflow/imageflow_tool" diagnose --self-test

        #export STACK_UID= $(docker-cloud stack up --name flow3 -f docker-solo.yaml)
        #printf "%s" "${STACK_UID}"

            docker-cloud stack up --name flow-tiny
            docker-cloud stack update flow-tiny
        )
	else
        docker build -t "imazen/imageflow_server" "$(pwd)"
        docker push  imazen/imageflow_server
        #docker run --rm "imazen/${IMAGE_NAME}" sudo "/home/conan/imageflow/imageflow_tool" diagnose --self-test

        #export STACK_UID= $(docker-cloud stack up --name flow3 -f docker-solo.yaml)
        #printf "%s" "${STACK_UID}"
        docker-cloud stack redeploy flow3
	fi
)



