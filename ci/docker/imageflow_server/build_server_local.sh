#!/bin/bash
set -e

(
	cd ../../..
	(
		cd imageflow_tool
		cargo build --release --bin imageflow_tool
	)
	(
		cd imageflow_server
		cargo build --release --bin imageflow_server
	)
)
./copy_server_from_local.sh