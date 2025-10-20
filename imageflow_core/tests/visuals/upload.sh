#!/bin/bash

# Uploads files from to_upload.txt to s3
# https://s3-us-west-2.amazonaws.com/imageflow-resources/visual_test_checksums/
# to_upload.txt contains the filenames 
# Loop through to_upload.txt and upload each file to s3
# The files are in the same directory as to_upload.txt (tests/visuals) and upload.sh
# Regardless of working directory, upload.sh will upload the files from the same directory as to_upload.txt

# subshell

# if aws command is present, upload files to s3

echo "You probably want to run aws configure first"

if command -v aws --version &> /dev/null; then

    ANY_FAILED=0
    cd $(dirname "$0")
    while read -r line; do
        echo "Uploading $line"
        aws s3 cp ./"$line" s3://imageflow-resources/visual_test_checksums/
        if [ $? -ne 0 ]; then
            ANY_FAILED=1
        fi
    done < to_upload.txt

    if [ $ANY_FAILED -eq 0 ]; then
        rm missing_on_s3.txt
        rm to_upload.txt
    fi
else
    echo "aws not installed, try: sudo snap install aws-cli --classic  then aws configure"
    exit 1
fi
