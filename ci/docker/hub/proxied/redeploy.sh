#!/bin/bash
set -e

#(cd config_only_image && ./push.sh)
#(cd docker-gen-cloud && ./push.sh)


docker-cloud stack update -f stackfile.yml imageflow-proxied
docker-cloud stack redeploy imageflow-proxied # --not-reuse-volumes - counts against weekly limit on letsencrypt, only as needed!
docker-cloud stack inspect imageflow-proxied