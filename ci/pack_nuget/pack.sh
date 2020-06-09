#!/bin/bash
set -e #Exit on failure.

# REQUIRES PACKAGE_SUFFIX
# REQUIRES NUGET_RUNTIME
# REQUIRES CI_TAG

if [[ "$1" == "tool" ]]; then
    export NUGET_PACKAGE_NAME=Imageflow.NativeTool.${PACKAGE_SUFFIX}
else
    export NUGET_PACKAGE_NAME=Imageflow.NativeRuntime.${PACKAGE_SUFFIX}
fi


if [[ "$CI_TAG" == 'v'* ]]; then
	export NUGET_PACKAGE_VERSION="${CI_TAG#"v"}"
else
	echo "CI_TAG not set; skipping nuget package upload"
	exit 0
fi 

export NUGET_COMBINED_NAME="$NUGET_PACKAGE_NAME.$NUGET_PACKAGE_VERSION"

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
	PROPS_PATH="build/net45/${NUGET_PACKAGE_NAME}.targets"
	NUGET_OUTPUT_DIR="${SCRIPT_DIR}/../../artifacts/nuget"
	NUGET_OUTPUT_FILE="${NUGET_OUTPUT_DIR}/${NUGET_COMBINED_NAME}.nupkg"
	echo RELEASE_DIR=${RELEASE_DIR}

	mkdir -p "${NUGET_OUTPUT_DIR}" || true


	if [[ "${NUGET_RUNTIME}" == *'win'* ]]; then
		LIB_NAME=imageflow.dll
		TOOL_NAME=imageflow_tool.exe
	elif [[ "${NUGET_RUNTIME}" == *'osx'* ]]; then
		LIB_NAME=libimageflow.dylib
		TOOL_NAME=imageflow_tool
	else
		LIB_NAME=libimageflow.so
		TOOL_NAME=imageflow_tool
	fi

	mkdir -p lib/netstandard1.0
	echo "" > lib/netstandard1.0/_._

	mkdir -p "$RUNTIME_DIR"
	if [[ "$1" == "tool" ]]; then
	    cp "${RELEASE_DIR}${TOOL_NAME}" "${RUNTIME_DIR}${TOOL_NAME}"
	else
	    cp "${RELEASE_DIR}${LIB_NAME}" "${RUNTIME_DIR}${LIB_NAME}"
	fi


	SED_NUGET_PACKAGE_NAME="$(echo $NUGET_PACKAGE_NAME | sed -e 's/[\/&]/\\&/g')"
	SED_NUGET_PACKAGE_VERSION="$(echo $NUGET_PACKAGE_VERSION | sed -e 's/[\/&]/\\&/g')"
	SED_NUGET_LIBFILE="$(echo $RUNTIME_DIR$LIB_NAME | sed -e 's/[\/&]/\\&/g' | sed -e 's/\//\\/g')" # fix slashes too


	if [[ "${NUGET_RUNTIME}" == *'win'* ]]; then
		
		if [[ "${NUGET_RUNTIME}" == *'x64'* ]]; then
			# add props
			mkdir -p build/net45
			cat ../../imageflow_x64.targets | sed -e "s/:rid:/$NUGET_RUNTIME/g" > "$PROPS_PATH"
		elif [[ "${NUGET_RUNTIME}" == *'x86'* ]]; then
			# add props
			mkdir -p build/net45
			cat ../../imageflow_x86.targets | sed -e "s/:rid:/$NUGET_RUNTIME/g" > "$PROPS_PATH"
		fi
	fi

    if [[ "$1" == "tool" ]]; then
	    NUSPEC_NAME="native_tool.nuspec"
	else
	    NUSPEC_NAME="native.nuspec"
	fi

	cat ../../${NUSPEC_NAME} \
		| sed -e "s/:id:/${SED_NUGET_PACKAGE_NAME}/g" \
		 | sed -e "s/:version:/${SED_NUGET_PACKAGE_VERSION}/g" > "${NUGET_PACKAGE_NAME}.nuspec"


	echo "${NUGET_PACKAGE_NAME}.nuspec:"
    cat "${NUGET_PACKAGE_NAME}.nuspec"
    echo
    
    rm "${NUGET_OUTPUT_FILE}" || true 
    zip -r "${NUGET_OUTPUT_FILE}" . || 7z a -tzip "${NUGET_OUTPUT_FILE}" "*"
    echo  "${NUGET_OUTPUT_FILE} packed"
   
)
