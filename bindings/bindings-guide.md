# Imageflow Language Binding Generation Guide

## Other essential files to read in

bindings/bindings-guide.md
bindings/bindings-openapi-guide.md
bindings/bindings-ci-guide.md
bindings/bindings-json-guide.md
bindings/bindings-dev-guide.md

## Overview

This guide explains the process for generating language-specific client bindings for the Imageflow API. The goal of our binding generation system is to be consistent, repeatable, and easy to maintain. To achieve this, we use a script-driven workflow that runs inside a monolithic Docker container, ensuring that the environment is identical for every language and on every machine.

The process is orchestrated by a single shell script and is designed to be the single source of truth for building the native library, generating the OpenAPI schema, generating the client code, and running smoke tests.

## Core Components

To understand the workflow, you must be familiar with these key files and directories:

- **`schema.json`**: This is the OpenAPI v3 schema that defines the Imageflow API. It is not written by hand; instead, it is generated directly from the Rust source code by a test. This ensures the API documentation is always in sync with the implementation.

- **`libimageflow.so`**: The native shared library, compiled from the Rust `imageflow_abi` crate. All language bindings are ultimately wrappers around this library.

- **`scripts/test_binding_generation.sh`**: The main entry point for the entire workflow. This script handles everything from building the native library to running smoke tests for a given language.

- **`scripts/generate_binding.sh`**: This script contains the core logic for invoking the OpenAPI Generator for each supported language. It is called by the main test script.

- **`scripts/install_binding_deps.sh`**: A setup script that installs all system-level dependencies required by the workflow, such as the OpenAPI Generator `.jar` file.

- **`bindings/docker/Dockerfile`**: A monolithic Dockerfile that defines the build environment for all language bindings. It contains the runtimes and tools for every supported language (e.g., Node.js, Ruby, etc.).

- **`docker/builder/Dockerfile`**: A separate, minimal Dockerfile used exclusively for compiling the native Rust library (`libimageflow.so`).

- **`bindings/imageflow-[language]/`**: The output directory for each generated language binding. These directories are created by the test script and should be in `.gitignore`.

## The Workflow in Detail

The end-to-end process is executed by running `./scripts/test_binding_generation.sh <language>`.

1.  **Stage 1: Build Native Library & Schema**
    - The script first builds the `imageflow-builder` Docker image.
    - It then runs a container from this image to compile `libimageflow.so` via `cargo build --release`.
    - Finally, it runs the container again to execute `cargo test --features schema-export --test schema`, which generates the master `schema.json` file in the project root.

2.  **Stage 2: Generate Language Bindings**
    - The script builds the monolithic `imageflow-binding-generator` Docker image.
    - It cleans any previous build artifacts and creates the output directory (e.g., `bindings/imageflow-typescript`).
    - It runs the generator container, which executes `scripts/generate_binding.sh`.
    - This script calls the `openapi-generator-cli.jar` with the correct generator name and configuration options for the target language.
    - All output from the generator is captured in `generator.log` and `generator.err` inside the output directory.

3.  **Stage 3: Run Smoke Test**
    - The script executes a smoke test to validate the generated code. This test is language-specific and typically runs inside the generator container.
    - For example, the TypeScript test patches `tsconfig.json`, runs `npm install`, and then `npm run build`.
    - The Ruby test runs `bundle install` and `bundle exec rspec`.

## How to Add a New Language

Follow these steps to add bindings for a new language (e.g., Go):

1.  **Update the Monolithic Dockerfile**: Add the necessary toolchain for the new language (e.g., the Go compiler) to `bindings/docker/Dockerfile`.

2.  **Research the Generator**: Use the `config-help` command to find the correct generator name and learn its configuration options:
    ```bash
    docker run --rm imageflow-binding-generator java -jar /usr/local/lib/openapi-generator-cli.jar config-help -g go
    ```

3.  **Update the Generation Script**: Add a new `case` statement for your language in `scripts/generate_binding.sh`. This is where you will add the `java -jar ...` command with the correct arguments you found in the previous step.

4.  **Update the Test Script**: Add a new `case` statement for your language in `scripts/test_binding_generation.sh`. This will define the smoke test for the new bindings.

5.  **Implement the Smoke Test**: Create the necessary test files and configuration (e.g., a simple Go program that imports and uses the generated client) and ensure your test script command executes it correctly.

## Debugging Tips

The most common source of issues is the OpenAPI generator itself.

- **Always check `generator.log` and `generator.err`**. The test script is configured to always `cat` these files after a generation attempt. They contain the full output from the Java process and are the most important source for debugging information.
- If a model name conflicts with a language primitive (like our `string` issue in TypeScript), you will need to find the correct mapping option (e.g., `--type-mappings`, `--model-name-mappings`, etc.) to resolve it. The `config-help` command is your best tool for this.
