FROM imazen/imageflow_base_os

MAINTAINER Lilith River

ARG IMAGEFLOW_DOWNLOAD_URL_TAR_GZ
ARG DOCKER_TAG

RUN if [ -z "${IMAGEFLOW_DOWNLOAD_URL_TAR_GZ}" ]; then echo "IMAGEFLOW_DOWNLOAD_URL_TAR_GZ not set - should be $(git rev-parse HEAD). Exiting." && exit 1; else echo "IMAGEFLOW_DOWNLOAD_URL_TAR_GZ=${IMAGEFLOW_DOWNLOAD_URL_TAR_GZ}"; fi


RUN mkdir nightly && cd nightly && wget -nv -O ifs.tar.gz ${IMAGEFLOW_DOWNLOAD_URL_TAR_GZ} \
    && tar xvzf ifs.tar.gz && mv ./imageflow_tool ../ && cd .. && rm -rf nightly


ENTRYPOINT ["/home/imageflow/imageflow_tool"]
CMD ["help"]
