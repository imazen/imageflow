#!/bin/bash

cd ${TRAVIS_BUILD_DIR}

echo "Install lcov"
# install latest LCOV
wget http://ftp.de.debian.org/debian/pool/main/l/lcov/lcov_1.11.orig.tar.gz
sudo tar xf lcov_1.11.orig.tar.gz
sudo make -C lcov-1.11/ install

#install lcov to coveralls conversion + upload tool
#crashes on darwin
sudo apt-get update
sudo apt-get install rubygems-integration -y
ls /usr/bin/g*
sudo /usr/bin/gem install coveralls-lcov

pwd
echo "*******  Cleaning cov **************"
sudo chmod -R a+rw .
lcov --directory ./build --capture --output-file coverage.info # capture coverage info
lcov --remove coverage.info 'tests/*' '.conan/*' '/usr/*' --output-file coverage.info # filter out system and test code
lcov --list coverage.info # debug before upload

echo "******* Uploading to coveralls **************"
coveralls-lcov --repo-token=${COVERALLS_TOKEN} coverage.info # uploads to coveralls
