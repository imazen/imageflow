# Imageflow Bindings Generation & FFI Smoke Test: Status

This document tracks the status and plan for generating language bindings for Imageflow.

## Current Strategy

- **Model-Only Generation**: We will use the OpenAPI schema to generate only the data models (classes, enums, structs) for each target language. These models will be treated as internal implementation details.
- **Manual FFI & Public API**: The Foreign Function Interface (FFI) layer and the public-facing API for each language will be implemented manually. This gives us full control over the public API's design, making it more idiomatic and stable.
- **.NET as a Template**: The manually-crafted .NET bindings located in `bindings/imageflow-dotnet` will serve as the reference implementation and template for other languages.
- **Unified Workflow**: A set of scripts in `bindings/scripts/` provides a unified, multi-mode workflow (local & Docker) for building the native library, generating models, and running smoke tests.

## Task List

- [x] Review `bindings/bindings-guide.md` for workflow overview
- [x] Review `scripts/test_binding_generation.sh` for smoke test process
- [x] Review `scripts/generate_binding.sh` for binding generation details
- [x] Design and implement Docker-free local workflow for binding generation and smoke testing (non-Go language)
- [x] Design and implement Docker-based workflow for binding generation and smoke testing (non-Go language)
- [x] Integrate both workflows into a unified, DRY multi-mode system (local, Docker, CI)
- [x] Ensure robust exit code handling and error reporting in all modes
- [x] Use local cargo build --release --crate imageflow_abi and openapi-generator for non-Go language
- [x] Document Docker-free, Docker-based, and CI workflows for local development and maintenance
- [ ] Implement FFI context creation and disposal
- [ ] Implement version endpoint smoke test
- [ ] Update smoke test to validate both context and version endpoint
- [x] Move and refactor scripts into /bindings/scripts/
- [x] Update build scripts to use cargo --target-dir and output native libraries to a predictable location
- [x] Switch binding generation to use the @openapitools/openapi-generator-cli npm wrapper
- [ ] Update workflow/scripts for model-only generation.
- [ ] Implement manual FFI layer and public API for each language, referencing .NET bindings as a template.
- [ ] Re-export portions of generated models suitable for public API.
- [ ] Update and maintain this document (`bindings/bindings-status.md`) with plan and progress.
- [ ] Establish a workflow for automatically triggering AI agents to update language bindings when the OpenAPI schema changes.

## Current Goal

Update the workflow and scripts to support the new model-only generation strategy.
