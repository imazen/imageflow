#!/bin/bash
set -e

docker-cloud stack up -n imageflow-proxied -f stackfile.yml
docker-cloud stack inspect imageflow-proxied