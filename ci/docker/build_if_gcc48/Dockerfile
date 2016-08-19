FROM ubuntu:trusty

MAINTAINER Nathanael Jones

#Trusty comes with python3, for some reason. 

RUN rm -rf /var/lib/apt/lists/* && apt-get update \
  && apt-get install --no-install-recommends -y \
  apt-utils python3-pip python3-setuptools sudo build-essential wget git nasm dh-autoreconf pkg-config curl ccache libpng-dev libssl-dev ca-certificates \
  && pip3 install --upgrade pip \
  && rm -rf /var/lib/apt/lists/* && rm -rf /usr/share/doc

#RUN dpkg --add-architecture i386 && rm -rf /var/lib/apt/lists/* && apt-get update && apt-get install --no-install-recommends -y libc6-dev-i386 g++-multilib vim && rm -rf /var/lib/apt/lists/*

# Install CMake 3.4.1 (minimal)
# Mirrored for speed from https://cmake.org/files/v3.4/cmake-3.4.1-Linux-x86_64.tar.gz
RUN wget https://s3.amazonaws.com/public-unit-test-resources/cmake-3.4.1-Linux-x86_64.tar.gz \
    && tar -xzf cmake-3.4.1-Linux-x86_64.tar.gz \
    && cp cmake-3.4.1-Linux-x86_64/bin/cmake /usr/bin/cmake \
    && cp cmake-3.4.1-Linux-x86_64/bin/ctest /usr/bin/ctest \
    && cp -fR cmake-3.4.1-Linux-x86_64/share/* /usr/share \
    && rm -rf cmake-3.4.1-Linux-x86_64 && rm cmake-3.4.1-Linux-x86_64.tar.gz

# Install DSSIM (minimal)
RUN wget -O dssim.tar.gz https://github.com/pornel/dssim/archive/c6ad29c5a2dc37d8610120486f09eda145621c84.tar.gz \
    && tar xvzf dssim.tar.gz && rm dssim.tar.gz && mv dssim-c6ad29c5a2dc37d8610120486f09eda145621c84 dssim \
    && cd dssim && make && sudo cp bin/dssim /usr/local/bin/dssim && cd .. && rm -rf dssim 

#Install Rust 1.11
RUN mkdir /rust | true
WORKDIR /rust

RUN RUST_ARCHIVE=rust-1.11.0-x86_64-unknown-linux-gnu.tar.gz &&\
    RUST_DOWNLOAD_URL=https://static.rust-lang.org/dist/$RUST_ARCHIVE &&\
    curl -fsOSL $RUST_DOWNLOAD_URL \
    && curl -s $RUST_DOWNLOAD_URL.sha256 | sha256sum -c - \
    && tar -C /rust -xzf $RUST_ARCHIVE --strip-components=1 \
    && rm $RUST_ARCHIVE \
    && ./install.sh --without=rust-docs && sudo rm -rf {/rust/rust-docs,/usr/doc,/usr/share/doc,/usr/share/man} \
    && sudo rm -rf /rust \
    && ln -sf -t /usr/local/lib/ /usr/local/lib/rustlib/x86_64-unknown-linux-gnu/lib/*.so
    

ENV PATH=/usr/lib/ccache:$PATH

RUN pip install conan

RUN groupadd 1001 -g 1001 &&\
    groupadd 1000 -g 1000 &&\
    useradd -ms /bin/bash conan -g 1001 -G 1000 &&\
    echo "conan:conan" | chpasswd && adduser conan sudo &&\
    echo "conan ALL= NOPASSWD: ALL\n" >> /etc/sudoers &&\
    mkdir -p /home/conan/.conan && sudo chown conan: /home/conan/.conan

USER conan
WORKDIR /home/conan/imageflow
