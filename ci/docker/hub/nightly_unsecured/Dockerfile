FROM ubuntu:xenial

MAINTAINER Nathanael Jones 

ARG SOURCE_COMMIT
ARG DOCKER_TAG

RUN if [ -z "${SOURCE_COMMIT}" ]; then echo "SOURCE_COMMIT not set; exiting" && exit 1; else echo "SOURCE_COMMIT=${SOURCE_COMMIT}"; fi

RUN apt-get update \
  && apt-get upgrade -y \
  && apt-get install --no-install-recommends -y \
    sudo libcurl4-openssl-dev  curl libssl-dev ca-certificates libpng-dev \
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

RUN mkdir nightly && cd nightly && wget -nv -O ifs.tar.gz https://s3-us-west-1.amazonaws.com/imageflow-nightlies/commits/${SOURCE_COMMIT}/linux64_glibc223.tar.gz \
    && tar xvzf ifs.tar.gz && mv ./imageflow_server ../ && cd .. && rm -rf nightly

EXPOSE 39876

ENV RUST_BACKTRACE 1

ENTRYPOINT ["/home/imageflow/imageflow_server"]
CMD ["start", "--demo"]
