#!/bin/bash
set -e

docker-cloud stack update -f docker-proxied.yml test-proxy
docker-cloud stack redeploy test-proxy
docker-cloud stack inspect test-proxy