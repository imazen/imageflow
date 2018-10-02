FROM imazen/imageflow_base_os

MAINTAINER Lilith River

ARG SOURCE_COMMIT
ARG DOCKER_TAG

RUN if [ -z "${SOURCE_COMMIT}" ]; then echo "SOURCE_COMMIT not set; exiting" && exit 1; else echo "SOURCE_COMMIT=${SOURCE_COMMIT}"; fi


RUN mkdir nightly && cd nightly && wget -nv -O ifs.tar.gz https://s3-us-west-1.amazonaws.com/imageflow-nightlies/commits/${SOURCE_COMMIT}/linux64_glibc227.tar.gz \
    && tar xvzf ifs.tar.gz && mv ./imageflow_server ../ && cd .. && rm -rf nightly

EXPOSE 39876

ENTRYPOINT ["/home/imageflow/imageflow_server"]
CMD ["start", "--demo"]
