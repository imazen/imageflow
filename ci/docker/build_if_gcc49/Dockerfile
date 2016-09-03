FROM ubuntu:vivid

MAINTAINER Nathanael Jones

RUN rm -rf /var/lib/apt/lists/* && apt-get update \
  && apt-get install --no-install-recommends -y \
  apt-utils sudo build-essential wget  git nasm dh-autoreconf pkg-config curl ccache libpng-dev libssl-dev ca-certificates \
  libcurl4-openssl-dev libelf-dev libdw-dev python2.7-minimal \
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

# Ensure python2 is available for kcov
RUN command -v python2 >/dev/null 2>&1 || sudo ln -sf /usr/bin/python2.7 /usr/bin/python2

# Install kcov

RUN wget -O kcov.tar.gz https://github.com/SimonKagstrom/kcov/archive/master.tar.gz \
    && tar xvzf kcov.tar.gz && rm kcov.tar.gz && mv kcov-master kcov \
    && mkdir kcov/build \
    && cd kcov/build \
    && cmake .. \
    && make \
    && sudo make install \
    && cd ../.. \
    && rm -rf kcov \
    && rm -rf /usr/local/share/man

# Install lcov

RUN wget -O lcov.tar.gz http://ftp.de.debian.org/debian/pool/main/l/lcov/lcov_1.11.orig.tar.gz \
    && tar xvzf lcov.tar.gz && rm lcov.tar.gz && mv lcov-1.11 lcov \
    && sudo make -C lcov/ install \
    && rm -rf lcov && rm -rf /usr/share/man


ENV PATH=/usr/lib/ccache:$PATH

# Install PIP and conan
RUN rm -rf /var/lib/apt/lists/* && apt-get update \
  && apt-get install --no-install-recommends -y \
  python3-minimal python3-pip python3-setuptools \
  && pip3 install conan \
  && rm -rf /var/lib/apt/lists/* \
  && rm -rf {/usr/share/doc,/usr/share/man,/var/cache,/usr/doc,/usr/local/share/doc,/usr/local/share/man}

# Install valgrind
RUN rm -rf /var/lib/apt/lists/* && apt-get update \
  && apt-get install --no-install-recommends -y \
  valgrind \
  && rm -rf /var/lib/apt/lists/* \
  && bash -c 'rm -rf {/usr/share/doc,/usr/share/man,/var/cache,/usr/doc,/usr/local/share/doc,/usr/local/share/man}' \
  && bash -c 'rm -f /usr/lib/valgrind/*-x86-*' \
  && bash -c 'rm -f /usr/lib/valgrind/{power,mips,s390,arm,32bit,i386}*' 




RUN groupadd 1001 -g 1001 &&\
    groupadd 1000 -g 1000 &&\
    useradd -ms /bin/bash conan -g 1001 -G 1000 &&\
    echo "conan:conan" | chpasswd && adduser conan sudo &&\
    echo "conan ALL= NOPASSWD: ALL\n" >> /etc/sudoers &&\
    mkdir -p /home/conan/.conan && sudo chown conan: /home/conan/.conan

USER conan

ENV PATH=/home/conan/.cargo/bin:$PATH

#Install Rust stable plus x86_64-unknown-linux-musl
# RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain stable -v \
#    && rustup target add x86_64-unknown-linux-musl \
#    && HI=$(rustup which rustc) && HI=${HI%/bin/rustc} && export TOOLCHAIN_DIR=$HI && echo TOOLCHAIN_DIR=$TOOLCHAIN_DIR \
#    && sudo rm -rf $TOOLCHAIN_DIR/share/doc \
#    && sudo rm -rf $TOOLCHAIN_DIR/share/man \
#    && ln -sf -t $TOOLCHAIN_DIR/lib/ $TOOLCHAIN_DIR/lib/rustlib/x86_64-unknown-linux-gnu/lib/*.so \
#    && rustup show \
#    && rustc -V

#Install nightly Rust and make default (add x86_64-unknown-linux-musl)

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly -v \
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
