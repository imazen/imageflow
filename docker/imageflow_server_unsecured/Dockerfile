FROM imazen/imageflow_base_os

# Obsolete! Use https://github.com/imazen/imageflow-dotnet-server instead


MAINTAINER Lilith River

ARG IMAGEFLOW_DOWNLOAD_URL_TAR_GZ
ARG DOCKER_TAG

RUN if [ -z "${IMAGEFLOW_DOWNLOAD_URL_TAR_GZ}" ]; then echo "IMAGEFLOW_DOWNLOAD_URL_TAR_GZ not set; exiting" && exit 1; else echo "IMAGEFLOW_DOWNLOAD_URL_TAR_GZ=${IMAGEFLOW_DOWNLOAD_URL_TAR_GZ}"; fi


RUN mkdir nightly && cd nightly && wget -nv -O ifs.tar.gz ${IMAGEFLOW_DOWNLOAD_URL_TAR_GZ} \
    && tar xvzf ifs.tar.gz && mv ./imageflow_server ../ && cd .. && rm -rf nightly

RUN sudo chown -R imageflow /home/imageflow/

EXPOSE 39876

ENTRYPOINT ["/home/imageflow/imageflow_server"]
CMD ["start", "--demo"]
