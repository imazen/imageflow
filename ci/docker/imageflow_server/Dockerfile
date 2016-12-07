FROM ubuntu:xenial

MAINTAINER Nathanael Jones 


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
    useradd -ms /bin/bash conan -g 1001 -G 1000 &&\
    echo "conan:conan" | chpasswd && adduser conan sudo &&\
    echo "conan ALL= NOPASSWD: ALL\n" >> /etc/sudoers &&\
    mkdir -p /home/conan/.conan && sudo chown conan: /home/conan/.conan

USER conan

WORKDIR /home/conan/imageflow

COPY bin/imageflow_tool /home/conan/imageflow/imageflow_tool
COPY bin/imageflow_server /home/conan/imageflow/imageflow_server