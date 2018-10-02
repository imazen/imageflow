#!/bin/bash
set -e

echo "int main() { return 0; }" | gcc -x c -o true -

docker build -t "imazen/nginx_template" . 

docker push imazen/nginx_template