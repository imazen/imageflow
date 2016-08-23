#!/bin/bash

mkdir source_images
cd source_images
wget -nc  http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/turtleegglarge.jpg
wget -nc  http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/rings2.png
wget -nc  http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/gamma_dalai_lama_gray.jpg
wget -nc  http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/u1.jpg
wget -nc  http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg
wget -nc  http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/u6.jpg
wget -nc  http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/premult_test.png
wget -nc  http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/gamma_test.jpg
cd ..
