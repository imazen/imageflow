#!/bin/bash
set -e
(
	cd imageflow_server
	./copy_server_from_gcc54.sh
	./build.sh
)