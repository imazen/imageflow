FROM ubuntu:xenial

RUN  apt-get update \
  && apt-get upgrade -y \
  && apt-get install --no-install-recommends -y \
  sudo build-essential nasm dh-autoreconf pkg-config ca-certificates \
  git zip curl libpng-dev libssl-dev wget libc6-dbg  \
  libcurl4-openssl-dev libelf-dev libdw-dev apt-transport-https \
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


