FROM ubuntu:bionic

MAINTAINER Lilith River

ARG BASE_OS_SOURCE_COMMIT
ARG BASE_OS_DOCKER_TAG

RUN if [ -z "${BASE_OS_SOURCE_COMMIT}" ]; then echo "BASE_OS_SOURCE_COMMIT not set; exiting" && exit 1; else echo "BASE_OS_SOURCE_COMMIT=${BASE_OS_SOURCE_COMMIT}"; fi

RUN apt-get update \
  && apt-get upgrade -y \
  && apt-get install --no-install-recommends -y \
    sudo wget libcurl4-openssl-dev curl libssl-dev ca-certificates libpng-dev \
  && apt-get clean -y \
  && apt-get autoremove -y \
  && rm -rf /var/lib/apt/lists/* \
  && bash -c 'rm -rf {/usr/share/doc,/usr/share/man,/var/cache,/usr/doc,/usr/local/share/doc,/usr/local/share/man}' \
  && bash -c 'rm -rf /tmp/*' \
  && bash -c 'rm -rf /var/tmp/*' \
  && sudo mkdir -p /var/cache/apt/archives/partial \
  && sudo touch /var/cache/apt/archives/lock \
  && sudo chmod 640 /var/cache/apt/archives/lock

RUN groupadd 1001 -g 1001 &&\
    groupadd 1000 -g 1000 &&\
    useradd -ms /bin/bash imageflow -g 1001 -G 1000 &&\
    echo "imageflow:imageflow" | chpasswd && adduser imageflow sudo &&\
    echo "imageflow ALL= NOPASSWD: ALL\n" >> /etc/sudoers

USER imageflow

WORKDIR /home/imageflow

ENV RUST_BACKTRACE 1
ENV BASE_OS_SOURCE_COMMIT="${BASE_OS_SOURCE_COMMIT}" BASE_OS_DOCKER_TAG="${BASE_OS_DOCKER_TAG}"
ENV BASE_OS_LAST_LAYER_UNIX_SECONDS="$(date +%s)"
