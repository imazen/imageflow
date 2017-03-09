FROM imazen/imageflow_base_os

MAINTAINER Nathanael Jones 

ARG SOURCE_COMMIT
ARG DOCKER_TAG

RUN if [ -z "${SOURCE_COMMIT}" ]; then echo "SOURCE_COMMIT not set; exiting" && exit 1; else echo "SOURCE_COMMIT=${SOURCE_COMMIT}"; fi && \
    if [[ "${SOURCE_COMMIT}" -ne "${BASE_OS_SOURCE_COMMIT}"]]; then echo "Mismatched images! ${BASE_OS_SOURCE_COMMIT} vs ${SOURCE_COMMIT}; exiting - push imazen/imageflow_base_os first" && exit 1; fi


RUN mkdir nightly && cd nightly && wget -nv -O ifs.tar.gz https://s3-us-west-1.amazonaws.com/imageflow-nightlies/commits/${SOURCE_COMMIT}/linux64_sandybridge_glibc223.tar.gz \
    && tar xvzf ifs.tar.gz && mv ./imageflow_tool ../ && cd .. && rm -rf nightly


ENTRYPOINT ["/home/imageflow/imageflow_tool"]
CMD ["help"]
