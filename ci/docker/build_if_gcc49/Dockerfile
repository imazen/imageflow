FROM ubuntu:vivid

MAINTAINER Nathanael Jones

RUN rm -rf /var/lib/apt/lists/* && apt-get update \
  && apt-get install --no-install-recommends -y \
  apt-utils python3-pip python3-setuptools sudo build-essential wget valgrind git nasm dh-autoreconf pkg-config curl ccache libpng-dev libssl-dev ca-certificates \
  && pip3 install --upgrade pip \
  && rm -rf /var/lib/apt/lists/* \
  && rm -rf {/usr/share/doc,/usr/share/man,/var/cache,/usr/doc,/usr/local/share/doc,/usr/local/share/man}


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

ENV PATH=/usr/lib/ccache:$PATH

RUN pip3 install conan

RUN groupadd 1001 -g 1001 &&\
    groupadd 1000 -g 1000 &&\
    useradd -ms /bin/bash conan -g 1001 -G 1000 &&\
    echo "conan:conan" | chpasswd && adduser conan sudo &&\
    echo "conan ALL= NOPASSWD: ALL\n" >> /etc/sudoers &&\
    mkdir -p /home/conan/.conan && sudo chown conan: /home/conan/.conan

USER conan

#Install Rustup & Rust stable plus x86_64-unknown-linux-musl

ENV PATH=/home/conan/.cargo/bin:$PATH

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain stable -v \
    && rustup target add x86_64-unknown-linux-musl \
    && HI=$(rustup which rustc) && HI=${HI%/bin/rustc} && export TOOLCHAIN_DIR=$HI && echo TOOLCHAIN_DIR=$TOOLCHAIN_DIR \
    && sudo rm -rf $TOOLCHAIN_DIR/share/doc \
    && sudo rm -rf $TOOLCHAIN_DIR/share/man \
    && ln -sf -t $TOOLCHAIN_DIR/lib/ $TOOLCHAIN_DIR/lib/rustlib/x86_64-unknown-linux-gnu/lib/*.so \
    && rustup show \
    && rustc -V

#Install nightly Rust and make default (add x86_64-unknown-linux-musl)

RUN rustup install nightly \
    && rustup default nightly \
    && rustup target add x86_64-unknown-linux-musl \
    && HI=$(rustup which rustc) && HI=${HI%/bin/rustc} && export TOOLCHAIN_DIR=$HI && echo TOOLCHAIN_DIR=$TOOLCHAIN_DIR \
    && sudo rm -rf $TOOLCHAIN_DIR/share/doc \
    && sudo rm -rf $TOOLCHAIN_DIR/share/man \
    && ln -sf -t $TOOLCHAIN_DIR/lib/ $TOOLCHAIN_DIR/lib/rustlib/x86_64-unknown-linux-gnu/lib/*.so \
    && rustup show \
    && rustc -V

RUN conan user

WORKDIR /home/conan/imageflow
