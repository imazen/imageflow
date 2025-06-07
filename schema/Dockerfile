# Use an official Rust image.
FROM rust:latest as builder

WORKDIR /usr/src/imageflow

# Copy the entire workspace
COPY . .

# Install build dependencies (like nasm for mozjpeg-sys)
# This might need adjustment based on specific C dependencies
RUN apt-get update && apt-get install -y nasm

# Build imageflow_tool with the schema-export feature enabled
# We build in release mode for potentially faster execution, though debug might be fine.
# Target the binary directly to avoid building everything.
RUN cargo build --release --package imageflow_tool --features imageflow_tool/schema-export

# --- Final Stage ---
# Use a smaller base image for the final stage if desired, or keep the rust one
# For now, keep the rust image to potentially run other tools (like doc generators)
FROM rust:latest

WORKDIR /app

# Copy the built binary from the builder stage
COPY --from=builder /usr/src/imageflow/target/release/imageflow_tool /usr/local/bin/imageflow_tool

# Copy the generation script
COPY schema/generate_schema.sh /app/generate_schema.sh
RUN chmod +x /app/generate_schema.sh

# Create the output directory within the container
RUN mkdir /output

# Set the entrypoint to the generation script
ENTRYPOINT ["/app/generate_schema.sh"] 