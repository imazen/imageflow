#!/bin/bash
set -e #Exit on failure.

# REQUIRES PACKAGE_SUFFIX
# REQUIRES NUGET_RUNTIME
# REQUIRES CI_TAG
export NUGET_PACKAGE_NAME=Imageflow.NativeRuntime.${PACKAGE_SUFFIX}


if [[ "$CI_TAG" == 'v'* ]]; then
	export NUGET_PACKAGE_VERSION="${CI_TAG#"v"}"
else
	echo "CI_TAG not set; skipping nuget package upload"
	exit 0
fi 

export NUGET_COMBINED_NAME="$NUGET_PACKAGE_NAME$NUGET_PACKAGE_VERSION"

SCRIPT_DIR="$(dirname "${BASH_SOURCE[0]}" )"
SCRIPT_DIR="$(cd "$SCRIPT_DIR"; pwd)"

STAGING_DIR="${SCRIPT_DIR}/staging"

mkdir -p "$STAGING_DIR" || true

( cd "$STAGING_DIR"
	
	rm -rf "./$NUGET_COMBINED_NAME"
	mkdir "$NUGET_COMBINED_NAME"
	cd "$NUGET_COMBINED_NAME"

	RELEASE_DIR="${SCRIPT_DIR}/../../target/release/"
	RUNTIME_DIR="runtimes/${NUGET_RUNTIME}/native/"
	PROPS_PATH="build/net45/${NUGET_PACKAGE_NAME}.props"
	echo RELEASE_DIR=$RELEASE_DIR


	if [[ "${NUGET_RUNTIME}" == *'win'* ]]; then
		LIB_NAME=imageflow.dll
	elif [[ "${NUGET_RUNTIME}" == *'osx'* ]]; then
		LIB_NAME=libimageflow.dylib
	else
		LIB_NAME=libimageflow.so
	fi
	mkdir -p lib/netstandard1.0
	echo "" > lib/netstandard1.0/_._

	mkdir -p "$RUNTIME_DIR"
	cp "${RELEASE_DIR}${LIB_NAME}" "${RUNTIME_DIR}${LIB_NAME}"

	if [[ "${NUGET_RUNTIME}" == *'win'* ]]; then
		
		if [[ "${NUGET_RUNTIME}" == *'x64'* ]]; then
			# add props
			mkdir -p build/net45
			cat ../../imageflow_x64.props | sed -e "s/:rid:/$NUGET_RUNTIME/g" > "$PROPS_PATH"
		elif [[ "${NUGET_RUNTIME}" == *'x86'* ]]; then
			# add props
			mkdir -p build/net45
			cat ../../imageflow_x86.props | sed -e "s/:rid:/$NUGET_RUNTIME/g" > "$PROPS_PATH"
		fi
	fi

	SED_NUGET_PACKAGE_NAME="$(echo $NUGET_PACKAGE_NAME | sed -e 's/[\/&]/\\&/g')"
	SED_NUGET_PACKAGE_VERSION="$(echo $NUGET_PACKAGE_VERSION | sed -e 's/[\/&]/\\&/g')"
	SED_NUGET_PACKAGE_NAME="$(echo $NUGET_PACKAGE_NAME | sed -e 's/[\/&]/\\&/g')"
	SED_NUGET_LIBFILE="$(echo $RUNTIME_DIR$LIB_NAME | sed -e 's/[\/&]/\\&/g' | sed -e 's/\//\\/g')" # fix slashes too
	PROPS_PATH="$(echo $PROPS_PATH | sed -e 's/\//\\/g')" #fix slashes
	PROPS="<file src=\"$PROPS_PATH\" target=\"$PROPS_PATH\" />"
	SED_NUGET_PROPS="$(echo $PROPS | sed -e 's/[\/&]/\\&/g')"
	
	
	# If these elements turn out to be needed, re-add them to native.nuspec
        #<file src="lib\netstandard1.0\_._" target="lib\netstandard1.0\_._" />
        #<file src=":libfile:" target=":libfile:" />
	cat ../../native.nuspec \
		| sed -e "s/:id:/${SED_NUGET_PACKAGE_NAME}/g" \
		 | sed -e "s/:version:/${SED_NUGET_PACKAGE_VERSION}/g" \
		 | sed -e "s/:libfile:/${SED_NUGET_LIBFILE}/" \
		 | sed -e "s/:props:/${SED_NUGET_PROPS}/" > "${NUGET_PACKAGE_NAME}.nuspec"

	echo "${NUGET_PACKAGE_NAME}.nuspec:"
    cat "${NUGET_PACKAGE_NAME}.nuspec"
    echo
   
    cd ..
    NUGET_OUTPUT_DIR="${SCRIPT_DIR}/../../artifacts/nuget"

    mkdir -p "${NUGET_OUTPUT_DIR}" || true
    zip -r "${NUGET_OUTPUT_DIR}/${NUGET_COMBINED_NAME}.nupkg" "${NUGET_COMBINED_NAME}" 
    echo  "${NUGET_OUTPUT_DIR}/${NUGET_COMBINED_NAME}.nupkg packed"
   
)
