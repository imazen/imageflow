#!/bin/bash

cargo build --release
cp ../target/release/flow-proto1 .

./flow-proto1 --version

wget -nc  https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/u1.jpg

./flow-proto1 -i u1.jpg -o u1__bench2.jpg -w 400 -h 400 --benchmark-threads=8 --benchmark-count=32

./flow-proto1 -i u1.jpg -o u1_2_bench2.jpg -w 400 -h 400 --benchmark-threads=1 --benchmark-count=12

./flow-proto1 -i u1.jpg -o u1_3_bench2.jpg -w 400 -h 400 --benchmark-threads=1 --benchmark-count=12 --incorrectgamma
