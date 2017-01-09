#/bin/bash
set -e

cd ./trusty

../mods/recreate.sh --size 16gb --image ubuntu-14-04-x64 --region nyc2 --ssh-keys "$(cat ../use_key.txt)"
sleep 15
../mods/load_imageflow.sh
../mods/agent.sh
./config.sh
../mods/ff_3000_demo.sh