FROM ubuntu:bionic

RUN  apt-get update \
  && apt-get upgrade -y \
  && apt-get install --no-install-recommends -y \
  sudo build-essential nasm dh-autoreconf pkg-config ca-certificates gnupg \
  git zip curl libpng-dev libssl-dev wget libc6-dbg  \
  libcurl4-openssl-dev libelf-dev libdw-dev apt-transport-https \
  cmake valgrind \
  && apt-get clean -y \
  && apt-get autoremove -y \
  && rm -rf /var/lib/apt/lists/* \
  && bash -c 'rm -rf {/usr/share/doc,/usr/share/man,/var/cache,/usr/doc,/usr/local/share/doc,/usr/local/share/man}' \
  && bash -c 'rm -rf /tmp/*' \
  && bash -c 'rm -rf /var/tmp/*' \
  && sudo mkdir -p /var/cache/apt/archives/partial \
  && sudo touch /var/cache/apt/archives/lock \
  && sudo chmod 640 /var/cache/apt/archives/lock


RUN wget -q https://packages.microsoft.com/config/ubuntu/16.04/packages-microsoft-prod.deb \
  && sudo dpkg -i packages-microsoft-prod.deb \
  && apt-get update \
  && apt-get install --no-install-recommends -y \
  ruby-dev ruby-bundler rubygems-integration \
  luajit \
  python3-minimal python3-pip python3-setuptools \
  && apt-get clean -y \
  && apt-get autoremove -y \
  && rm -rf /var/lib/apt/lists/* \
  && bash -c 'rm -rf {/usr/share/doc,/usr/share/man,/var/cache,/usr/doc,/usr/local/share/doc,/usr/local/share/man}' \
  && bash -c 'rm -rf /tmp/* || true' \
  && bash -c 'rm -rf /var/tmp/*' \
  && sudo mkdir -p /var/cache/apt/archives/partial \
  && sudo touch /var/cache/apt/archives/lock \
  && sudo chmod 640 /var/cache/apt/archives/lock



# Install lcov and  coveralls-lcov
RUN wget -nv  -O lcov.tar.gz http://ftp.de.debian.org/debian/pool/main/l/lcov/lcov_1.11.orig.tar.gz \
    && tar xvzf lcov.tar.gz && rm lcov.tar.gz && mv lcov-1.11 lcov \
    && sudo make -C lcov/ install \
    && rm -rf lcov && rm -rf /usr/share/man \
    && sudo gem install coveralls-lcov

# Install kcov
RUN wget -nv  -O kcov.tar.gz https://github.com/SimonKagstrom/kcov/archive/master.tar.gz \
    && tar xvzf kcov.tar.gz && rm kcov.tar.gz && mv kcov-master kcov \
    && mkdir kcov/build && cd kcov/build \
    && cmake .. && make && sudo make install \
    && cd ../.. && rm -rf kcov && rm -rf /usr/local/share/man


RUN groupadd 1001 -g 1001 &&\
    groupadd 1000 -g 1000 &&\
    useradd -ms /bin/bash imageflow -g 1001 -G 1000 &&\
    echo "imageflow:imageflow" | chpasswd && adduser imageflow sudo &&\
    echo "imageflow ALL= NOPASSWD: ALL\n" >> /etc/sudoers

USER imageflow

ENV PATH=/home/imageflow/.cargo/bin:$PATH


#Install stable Rust and make default
RUN RUSTVER="stable" && curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain $RUSTVER -v \
    && rustup default $RUSTVER \
    && HI=$(rustup which rustc) && HI=${HI%/bin/rustc} && export TOOLCHAIN_DIR=$HI && echo TOOLCHAIN_DIR=$TOOLCHAIN_DIR \
    && sudo rm -rf $TOOLCHAIN_DIR/share/doc \
    && sudo rm -rf $TOOLCHAIN_DIR/share/man \
    && sudo rm -rf /home/imageflow/.multirust/toolchains/${RUSTVER}-x86_64-unknown-linux-gnu/share/doc \
    && ln -sf -t $TOOLCHAIN_DIR/lib/ $TOOLCHAIN_DIR/lib/rustlib/x86_64-unknown-linux-gnu/lib/*.so \
    && rustup show \
    && rustc -V

RUN PKG_CONFIG_ALL_STATIC=1 cargo install --force --git=https://github.com/mozilla/sccache.git --features=s3 \
    && PKG_CONFIG_ALL_STATIC=1 cargo install dssim \
    && cargo install mdbook --no-default-features --features output --vers "^0.3" \
    && rm -rf ~/.cargo/registry

WORKDIR /home/imageflow/imageflow

MAINTAINER Lilith River

