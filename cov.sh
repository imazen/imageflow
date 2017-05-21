#!/bin/bash
set -e

rm -rf ./target/{c_cov,rust_cov,rust_coverage,c_coverage} || true

for i in ./target/debug/imageflow*-[0-9a-f][0-9a-f]*
do
	if [[ "$i" == *.* ]]; then 
		echo "Skipping $i"
	else
		echo "Covering $i"
		DIR="./target/rust_coverage/$(basename "$i")"
		mkdir -p "$DIR"
		kcov --exclude-pattern=/.cargo,/.conan,/usr,/tests --include-path=./c_components/lib,./imageflow_core/src,./imageflow_server/src,./imageflow_tool/src,./imageflow_types/src,./imageflow_helpers/src "$DIR" "$i"
	fi 
done

for i in ./c_components/build/bin/test_*
do
	if [[ "$i" == *.* ]]; then 
		echo "Skipping $i"
	else
		echo "Covering $i"
		DIR="./target/c_coverage/$(basename "$i")"
		mkdir -p "$DIR"
		kcov --exclude-pattern=/.cargo,/.conan,/usr,/tests "$DIR" "$i"
	fi 
done


echo "Merging $(ls ./target/c_coverage)"
kcov --merge ./target/c_cov ./target/coverage/*/

echo "Merging $(ls ./target/rust_coverage)"
kcov --merge ./target/rust_cov ./target/coverage/*/

if [[ "$CODECOV" == "True" ]]; then 
	echo "Uploading to codecov.io"
	bash <(curl -s https://codecov.io/bash) -s ./target/c_cov
	bash <(curl -s https://codecov.io/bash) -s ./target/rust_cov
fi 

#kcov --merge ./target/cov  /home/n/.docker_imageflow_caches/.docker_build_if_gcc48_x86-64/target/coverage/*/