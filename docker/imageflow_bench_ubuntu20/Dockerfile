FROM ubuntu:20.04

# libpng-dev is required for libpng-sys crate
# libssl-dev and pkg-config are required for SSL support
# nasm is required for libjpeg-turbo

RUN export DEBIAN_FRONTEND=noninteractive \
  && apt-get update \
  && apt-get upgrade -y \
  && apt-get install --no-install-recommends -y \
    sudo tzdata build-essential nasm dh-autoreconf pkg-config ca-certificates \
    git zip curl libpng-dev libssl-dev wget \
    libcurl4-openssl-dev libelf-dev libdw-dev parallel time imagemagick graphicsmagick pngcrush optipng \
  && ln -fs /usr/share/zoneinfo/America/Denver /etc/localtime \
  && dpkg-reconfigure --frontend noninteractive tzdata \
  && apt-get clean -y \
  && apt-get autoremove -y \
  && rm -rf /var/lib/apt/lists/* \
  && bash -c 'rm -rf {/usr/share/doc,/usr/share/man,/var/cache,/usr/doc,/usr/local/share/doc,/usr/local/share/man}' \
  && bash -c 'rm -rf /tmp/* || true' \
  && bash -c 'rm -rf /var/tmp/*' \
  && sudo mkdir -p /var/cache/apt/archives/partial \
  && sudo touch /var/cache/apt/archives/lock \
  && sudo chmod 640 /var/cache/apt/archives/lock

 RUN apt-get update \
   && sudo apt-get upgrade -y \
   && sudo apt-get install -y \
     libvips-dev libvips \
   && apt-get clean -y \
   && apt-get autoremove -y \
   && rm -rf /var/lib/apt/lists/* \
   && bash -c 'rm -rf {/usr/share/doc,/usr/share/man,/var/cache,/usr/doc,/usr/local/share/doc,/usr/local/share/man}' \
   && bash -c 'rm -rf /tmp/* || true' \
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

RUN gm version
RUN convert --version
RUN vipsthumbnail --vips-version

ENV PATH=/home/imageflow/.cargo/bin:$PATH

#Install stable Rust and make default
RUN RUSTVER="stable" && curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain $RUSTVER -v \
    && rustup default $RUSTVER \
    && HI=$(rustup which rustc) && HI=${HI%/bin/rustc} && export TOOLCHAIN_DIR=$HI && echo TOOLCHAIN_DIR=$TOOLCHAIN_DIR \
    && sudo rm -rf $TOOLCHAIN_DIR/share/doc \
    && sudo rm -rf $TOOLCHAIN_DIR/share/man \
    && sudo rm -rf /home/imageflow/.rustup/toolchains/${RUSTVER}-x86_64-unknown-linux-gnu/share/doc \
    && ln -sf -t $TOOLCHAIN_DIR/lib/ $TOOLCHAIN_DIR/lib/rustlib/x86_64-unknown-linux-gnu/lib/*.so \
    && rustup show \
    && rustc -V

WORKDIR /home/imageflow

#Install hyperfine
RUN wget https://github.com/sharkdp/hyperfine/releases/download/v1.9.0/hyperfine_1.9.0_amd64.deb \
    && sudo dpkg -i hyperfine_1.9.0_amd64.deb

# Install DSSIM
RUN cargo install dssim && cargo install oxipng

# Build Imageflow from source with AVX2 support (haswell), then delete everything except the binary
RUN cd /home/imageflow \
    && git clone https://github.com/imazen/imageflow \
    && cd /home/imageflow/imageflow \
    && git checkout v1.4.0-rc40 \
    && TARGET_CPU=haswell cargo build -p imageflow_tool_lib --release \
    && mkdir $HOME/bin  \
    && cp target/release/imageflow_tool $HOME/bin/imageflow_tool \
    && cd $HOME \
    && rm -rf $HOME/imageflow

RUN wget -nc --quiet  https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/u1.jpg

CMD ["thumbnail"]
ENTRYPOINT ["./bench.sh"]

MAINTAINER Lilith River

ADD bench.sh .
RUN sudo chmod +x $HOME/bench.sh