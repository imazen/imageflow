FROM ubuntu:xenial

MAINTAINER Nathanael Jones 


RUN apt-get update \
  && apt-get install --no-install-recommends -y \
  cmake \
  wget \
  nasm \
  curl \
  file \
  git \
  g++ \
  python \
  make \
  nano \
  ca-certificates \
  xz-utils \
  musl-tools \
  pkg-config \
  apt-file \
  xutils-dev \
  sudo \
  luajit \
  ruby-dev ruby-bundler rubygems-integration \
  python-minimal python-pip python-setuptools \
  && pip install conan \
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


ENV PATH=/home/conan/.cargo/bin:$PATH


# Compile C libraries with musl-gcc
ENV SSL_VER=1.0.2j \
    CURL_VER=7.51.0 \
    CC=musl-gcc \
    PREFIX=/usr/local \
    PATH=/usr/local/bin:$PATH \
    PKG_CONFIG_PATH=/usr/local/lib/pkgconfig



# #Install nightly Rust and make default
# RUN RUSTVER="nightly-2017-02-05" && curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain $RUSTVER -v \
#     && rustup default $RUSTVER \
#     && HI=$(rustup which rustc) && HI=${HI%/bin/rustc} && export TOOLCHAIN_DIR=$HI && echo TOOLCHAIN_DIR=$TOOLCHAIN_DIR \
#     && sudo rm -rf $TOOLCHAIN_DIR/share/doc \
#     && sudo rm -rf $TOOLCHAIN_DIR/share/man \
#     && sudo rm -rf /home/conan/.multirust/toolchains/${RUSTVER}-x86_64-unknown-linux-gnu/share/doc \
#     && ln -sf -t $TOOLCHAIN_DIR/lib/ $TOOLCHAIN_DIR/lib/rustlib/x86_64-unknown-linux-gnu/lib/*.so \
#     && rustup show \
#     && rustc -V

# # RUN sudo pip install conan --upgrade

RUN RUSTVER="nightly-2016-11-03" && curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain $RUSTVER -v \
	&& rustup target add x86_64-unknown-linux-musl 
RUN RUSTVER="nightly-2016-11-03" && rustup default $RUSTVER \
    && rustup show \
    && rustc -V \
    && echo "[build]\ntarget = \"x86_64-unknown-linux-musl\"" > ~/.cargo/config


RUN conan user

WORKDIR /home/conan/imageflow

RUN sudo apt-get update \
  && sudo apt-get install --no-install-recommends -y \
  dh-autoreconf \
  && sudo apt-get clean -y \
  && sudo apt-get autoremove -y \
  && sudo rm -rf /var/lib/apt/lists/* \
  && sudo bash -c 'rm -rf {/usr/share/doc,/usr/share/man,/var/cache,/usr/doc,/usr/local/share/doc,/usr/local/share/man}' \
  && sudo bash -c 'rm -rf /tmp/*' \
  && sudo bash -c 'rm -rf /var/tmp/*' \
  && sudo mkdir -p /var/cache/apt/archives/partial \
  && sudo touch /var/cache/apt/archives/lock \
  && sudo chmod 640 /var/cache/apt/archives/lock


# libpng-dev libssl-dev wget \
#   libcurl4-openssl-dev libelf-dev libdw-dev \

# RUN curl -sL http://www.openssl.org/source/openssl-$SSL_VER.tar.gz | tar xz && \
#     cd openssl-$SSL_VER && \
#     ./Configure no-shared --prefix=$PREFIX --openssldir=$PREFIX/ssl no-zlib linux-x86_64 && \
#     make depend 2> /dev/null && make -j$(nproc) && make install && \
#     cd .. && rm -rf openssl-$SSL_VER

# RUN curl https://curl.haxx.se/download/curl-$CURL_VER.tar.gz | tar xz && \
#     cd curl-$CURL_VER && \
#     ./configure --enable-shared=no --enable-static=ssl --enable-optimize --prefix=$PREFIX \
#       --with-ca-path=/etc/ssl/certs/ --with-ca-bundle=/etc/ssl/certs/ca-certificates.crt --without-ca-fallback && \
#     make -j$(nproc) && make install && \
#     cd .. && rm -rf curl-$CURL_VER

# SSL cert directories get overridden by --prefix and --openssldir
# and they do not match the typical host configurations.
# The SSL_CERT_* vars fix this, but only when inside this container
# musl-compiled binary must point SSL at the correct certs (muslrust/issues/5) elsewhere
# OPENSSL_ vars are backwards compat with older rust-openssl and are not needed with new versions of it
# ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt \
#     SSL_CERT_DIR=/etc/ssl/certs \
#     OPENSSL_LIB_DIR=$PREFIX/lib \
#     OPENSSL_INCLUDE_DIR=$PREFIX/include \
#     OPENSSL_DIR=$PREFIX \
#     OPENSSL_STATIC=1







# RUN apt-get update \
#   && apt-get install --no-install-recommends -y \
#   sudo build-essential dh-autoreconf pkg-config ca-certificates \
#   && apt-get clean -y \
#   && apt-get autoremove -y \
#   && rm -rf /var/lib/apt/lists/* \
#   && bash -c 'rm -rf {/usr/share/doc,/usr/share/man,/var/cache,/usr/doc,/usr/local/share/doc,/usr/local/share/man}' \
#   && bash -c 'rm -rf /tmp/*' \
#   && bash -c 'rm -rf /var/tmp/*' \
#   && sudo mkdir -p /var/cache/apt/archives/partial \
#   && sudo touch /var/cache/apt/archives/lock \
#   && sudo chmod 640 /var/cache/apt/archives/lock

# RUN apt-get update \
#   && apt-get install --no-install-recommends -y \
#   libpng-dev libssl-dev wget \
#   libcurl4-openssl-dev libelf-dev libdw-dev \
#   && apt-get clean -y \
#   && apt-get autoremove -y \
#   && rm -rf /var/lib/apt/lists/* \
#   && bash -c 'rm -rf {/usr/share/doc,/usr/share/man,/var/cache,/usr/doc,/usr/local/share/doc,/usr/local/share/man}' \
#   && bash -c 'rm -rf /tmp/*' \
#   && bash -c 'rm -rf /var/tmp/*' \
#   && sudo mkdir -p /var/cache/apt/archives/partial \
#   && sudo touch /var/cache/apt/archives/lock \
#   && sudo chmod 640 /var/cache/apt/archives/lock


# RUN VERS=1.2.8 && \
#     cd /home/rust/libs && \
#     curl -LO http://zlib.net/zlib-$VERS.tar.gz && \
#     tar xzf zlib-$VERS.tar.gz && cd zlib-$VERS && \
#     CC=musl-gcc ./configure --static --prefix=/usr/local/musl && \
#     make && sudo make install && \
#     cd .. && rm -rf zlib-$VERS.tar.gz zlib-$VERS


#     #https://github.com/clux/muslrust
