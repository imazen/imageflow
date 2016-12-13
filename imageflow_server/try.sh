#!/bin/bash
set -e


firefox http://localhost:3000 & cargo run --bin imageflow_server start & wait
