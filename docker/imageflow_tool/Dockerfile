FROM imazen/imageflow_base_os

MAINTAINER Lilith River

ARG SOURCE_COMMIT
ARG DOCKER_TAG

RUN if [ -z "${SOURCE_COMMIT}" ]; then echo "SOURCE_COMMIT not set - should be $(git rev-parse HEAD). Exiting." && exit 1; else echo "SOURCE_COMMIT=${SOURCE_COMMIT}"; fi


RUN mkdir nightly && cd nightly && wget -nv -O ifs.tar.gz https://argoyal.s3.us-east-2.amazonaws.com/commits/${SOURCE_COMMIT}/linux64_glibc227.tar.gz \
    && tar xvzf ifs.tar.gz && mv ./imageflow_tool ../ && cd .. && rm -rf nightly


ENTRYPOINT ["/home/imageflow/imageflow_tool"]
CMD ["help"]
