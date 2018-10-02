#!/bin/sh

PROXY_SERVICE=$PROXY_SERVICE_ENV_VAR

echo "Redeploying proxy service [${NGINX_PROXY_SERVICE}]..."
proxy=`docker-cloud service ps --status Running | grep "^${PROXY_SERVICE}" | awk '{print $2}'`
docker-cloud service redeploy $proxy
echo "Redeployed proxy service [${NGINX_PROXY_SERVICE}]"