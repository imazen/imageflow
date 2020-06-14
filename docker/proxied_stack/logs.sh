#!/bin/bash

docker-cloud stack inspect imageflow-proxied
docker-cloud service logs nginx-proxy
docker-cloud service logs nginx-gen
docker-cloud service logs letsencrypt-nginx-proxy