#!/bin/bash
set -e #Exit on failure.

echo "Doesn't work - portable installation of lua doesn't work with luaffi, which needs .pc files and an actual installation"

exit 3

LUA_VER="lua-5.2.4"
LUA_FFI_VER="abc638c9341025580099dcf77795c4b320ba0e63"


mkdir "lua_runtime" || true
(
	cd lua_runtime
	export PATH="$(pwd):${PATH}"
	export CPATH="$(pwd):${CPATH}"
	if [ ! -d "$LUA_VER" ]; then
		curl -R -O "http://www.lua.org/ftp/${LUA_VER}.tar.gz"
		tar zxf "${LUA_VER}.tar.gz"
	fi
	(
		cd "${LUA_VER}"
		if [[ "$(uname -s)" == 'Darwin' ]]; then
			make macosx test
		else
			make linux test
		fi
		cp "src/lua" "../"
		cp src/*.h "../"
	)

	REPO_NAME=luaffi
	if [ ! -d "$REPO_NAME" ]; then
		git clone "https://github.com/jmckaskill/${REPO_NAME}"
	fi
	(
		cd "${REPO_NAME}"
		git reset --hard "${LUA_FFI_VER}"
		if [[ "$(uname -s)" == 'Darwin' ]]; then
			make macosx
		else
			make
		fi
	)
)

