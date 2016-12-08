#!/bin/bash
set -e

NAME=statifier-1.7.4
tar xvzf ${NAME}.tar.gz
rm ${NAME}.tar.gz 
cd ${NAME} && make

echo "Your turn! - type sudo make install"