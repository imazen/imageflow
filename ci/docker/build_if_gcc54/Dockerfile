FROM ubuntu:xenial

MAINTAINER Nathanael Jones 

RUN apt-get update \
  && apt-get install --no-install-recommends -y \
  apt-utils sudo build-essential wget git nasm dh-autoreconf pkg-config curl ccache libpng-dev libssl-dev ca-certificates \
  rubygems-integration ruby \
  libcurl4-openssl-dev libelf-dev libdw-dev \
  valgrind \
  python-minimal python-pip python-setuptools \
  && pip install conan \
  && apt-get clean -y \
  && apt-get autoremove -y \
  && rm -rf /var/lib/apt/lists/* \
  && bash -c 'rm -rf {/usr/share/doc,/usr/share/man,/var/cache,/usr/doc,/usr/local/share/doc,/usr/local/share/man}' \
  && bash -c 'rm -rf /usr/lib/valgrind/*-x86-*' \
  && bash -c 'rm -rf /usr/lib/valgrind/{power,mips,s390,arm,32bit,i386}*' \
  && bash -c 'rm -rf /tmp/*' \
  && bash -c 'rm -rf /var/tmp/*' 


#Try to fix problems deleting /var/cache causes
RUN sudo mkdir -p /var/cache/apt/archives/partial && sudo touch /var/cache/apt/archives/lock && sudo chmod 640 /var/cache/apt/archives/lock

# Install CMake 3.4.1 (minimal)
# Mirrored for speed from https://cmake.org/files/v3.4/cmake-3.4.1-Linux-x86_64.tar.gz
RUN wget -nv https://s3.amazonaws.com/public-unit-test-resources/cmake-3.4.1-Linux-x86_64.tar.gz \
    && tar -xzf cmake-3.4.1-Linux-x86_64.tar.gz \
    && cp cmake-3.4.1-Linux-x86_64/bin/cmake /usr/bin/cmake \
    && cp cmake-3.4.1-Linux-x86_64/bin/ctest /usr/bin/ctest \
    && cp -fR cmake-3.4.1-Linux-x86_64/share/* /usr/share \
    && rm -rf cmake-3.4.1-Linux-x86_64 && rm cmake-3.4.1-Linux-x86_64.tar.gz

# Install DSSIM (minimal)
RUN wget -nv  -O dssim.tar.gz https://github.com/pornel/dssim/archive/c6ad29c5a2dc37d8610120486f09eda145621c84.tar.gz \
    && tar xvzf dssim.tar.gz && rm dssim.tar.gz && mv dssim-c6ad29c5a2dc37d8610120486f09eda145621c84 dssim \
    && cd dssim && make && sudo cp bin/dssim /usr/local/bin/dssim && cd .. && rm -rf dssim 


# Install lcov
RUN wget -nv  -O lcov.tar.gz http://ftp.de.debian.org/debian/pool/main/l/lcov/lcov_1.11.orig.tar.gz \
    && tar xvzf lcov.tar.gz && rm lcov.tar.gz && mv lcov-1.11 lcov \
    && sudo make -C lcov/ install \
    && rm -rf lcov && rm -rf /usr/share/man

#Install coveralls-lcov
RUN sudo gem install coveralls-lcov

# Install kcov
RUN wget -nv  -O kcov.tar.gz https://github.com/SimonKagstrom/kcov/archive/master.tar.gz \
    && tar xvzf kcov.tar.gz && rm kcov.tar.gz && mv kcov-master kcov \
    && mkdir kcov/build \
    && cd kcov/build \
    && cmake .. \
    && make \
    && sudo make install \
    && cd ../.. \
    && rm -rf kcov \
    && rm -rf /usr/local/share/man


RUN groupadd 1001 -g 1001 &&\
    groupadd 1000 -g 1000 &&\
    useradd -ms /bin/bash conan -g 1001 -G 1000 &&\
    echo "conan:conan" | chpasswd && adduser conan sudo &&\
    echo "conan ALL= NOPASSWD: ALL\n" >> /etc/sudoers &&\
    mkdir -p /home/conan/.conan && sudo chown conan: /home/conan/.conan

USER conan

ENV PATH=/usr/lib/ccache:$PATH
ENV PATH=/home/conan/.cargo/bin:$PATH


#Install nightly Rust and make default
RUN RUSTVER="nightly-2016-11-03" && curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain $RUSTVER -v \
    && rustup default $RUSTVER \
    && HI=$(rustup which rustc) && HI=${HI%/bin/rustc} && export TOOLCHAIN_DIR=$HI && echo TOOLCHAIN_DIR=$TOOLCHAIN_DIR \
    && sudo rm -rf $TOOLCHAIN_DIR/share/doc \
    && sudo rm -rf $TOOLCHAIN_DIR/share/man \
    && sudo rm -rf /home/conan/.multirust/toolchains/${RUSTVER}-x86_64-unknown-linux-gnu/share/doc \
    && ln -sf -t $TOOLCHAIN_DIR/lib/ $TOOLCHAIN_DIR/lib/rustlib/x86_64-unknown-linux-gnu/lib/*.so \
    && rustup show \
    && rustc -V


# RUN sudo pip install conan --upgrade

RUN conan user

WORKDIR /home/conan/imageflow
