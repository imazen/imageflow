FROM ubuntu:xenial

MAINTAINER Nathanael Jones 

RUN dpkg --add-architecture i386 && rm -rf /var/lib/apt/lists/* && apt-get update && apt-get install -y python-dev sudo build-essential pkg-config wget git vim libc6-dev-i386 g++-multilib libpng-dev libgmp-dev libmpfr-dev libmpc-dev libc6-dev nasm dh-autoreconf rubygems-integration ccache ruby curl libssl-dev
# RUN rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/* 

RUN wget https://bootstrap.pypa.io/get-pip.py --no-check-certificate && python get-pip.py && pip install -U pip

# Install CMake 3
RUN wget https://cmake.org/files/v3.4/cmake-3.4.1-Linux-x86_64.tar.gz --no-check-certificate && tar -xzf cmake-3.4.1-Linux-x86_64.tar.gz && cp -fR cmake-3.4.1-Linux-x86_64/* /usr && rm -rf cmake-3.4.1-Linux-x86_64 && rm cmake-3.4.1-Linux-x86_64.tar.gz

# Install DSSIM
RUN wget -O dssim.tar.gz https://github.com/pornel/dssim/archive/c6ad29c5a2dc37d8610120486f09eda145621c84.tar.gz && \
tar xvzf dssim.tar.gz && mv dssim-c6ad29c5a2dc37d8610120486f09eda145621c84 dssim && cd dssim && \
make && sudo cp bin/dssim /usr/local/bin/dssim && cd .. && rm -rf dssim 

#Install coveralls-lcov
RUN sudo gem install coveralls-lcov

#Install Rust 1.11
RUN mkdir /rust | true
WORKDIR /rust

RUN RUST_ARCHIVE=rust-1.11.0-x86_64-unknown-linux-gnu.tar.gz &&\
    RUST_DOWNLOAD_URL=https://static.rust-lang.org/dist/$RUST_ARCHIVE &&\
    curl -fsOSL $RUST_DOWNLOAD_URL \
    && curl -s $RUST_DOWNLOAD_URL.sha256 | sha256sum -c - \
    && tar -C /rust -xzf $RUST_ARCHIVE --strip-components=1 \
    && rm $RUST_ARCHIVE \
    && ./install.sh

ENV PATH=/usr/lib/ccache:$PATH

RUN pip install conan

RUN groupadd 1001 -g 1001 &&\
    groupadd 1000 -g 1000 &&\
    useradd -ms /bin/bash conan -g 1001 -G 1000 &&\
    echo "conan:conan" | chpasswd && adduser conan sudo &&\
    echo "conan ALL= NOPASSWD: ALL\n" >> /etc/sudoers &&\
    mkdir -p /home/conan/.conan

USER conan
WORKDIR /home/conan
