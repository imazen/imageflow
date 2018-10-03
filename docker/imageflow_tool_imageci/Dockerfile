FROM imazen/imageflow_build_ubuntu18 as builder
# We expect user imageflow to have uid 100, and /home/imageflow/imageflow to exist
# We expect the build context to be the source checkout directory, preferably with the .git folder

USER imageflow

# We have to wrest ownership of /home/imageflow/imageflow, as for some reason Docker makes root owner
RUN sudo chown imageflow: /home/imageflow/imageflow || true
# Also set ownership of files copied in
COPY --chown=1000 . /home/imageflow/imageflow

WORKDIR /home/imageflow/imageflow

#RUN awk -F: '{printf "%s:%s\n",$1,$3}' /etc/passwd && ls -la


RUN cargo build --release --package imageflow_tool_lib --bin imageflow_tool

# Start over from a smaller (160MB) image
FROM imazen/imageflow_base_os
MAINTAINER Lilith River

WORKDIR /home/imageflow
COPY --from=builder /home/imageflow/imageflow/target/release/imageflow_tool .
RUN ./imageflow_tool diagnose --show-compilation-info
CMD ["./imageflow_tool"]