# Imageflow Bindings CI Guide

> **Scope** – This document describes the automation architecture for building, testing, versioning, and publishing *all* language bindings that sit in `bindings/`. It complements `bindings-dev-guide.md`, which focuses on how to *write* bindings. Here we focus on how to **build and ship** them.

## Table of Contents
1. [Repository Layout](#1-repository-layout)
2. [Schema Generation Architecture](#2-schema-generation-architecture)
3. [Binding Generation Workflow](#3-binding-generation-workflow)
4. [Docker Images](#4-docker-images)
5. [Language-Specific Shims](#5-language-specific-shims)
6. [Cross-Platform Constraints](#6-cross-platform-constraints)
7. [PR Automation Strategy](#7-pr-automation-strategy)
8. [Semantic Versioning](#8-semantic-versioning)
9. [Integration with Existing CI](#9-integration-with-existing-ci)
10. [Future Work](#10-future-work)

Note: When developing and testing, we are using WSL. Avoid powershell for bindings work.
---

## 1 Repository Layout

```text
/                         # monorepo root
├─ bindings/
│  ├─ templates/          # openapi-generator overrides per language
│  │  ├─ csharp/
│  │  ├─ node/
│  │  ├─ go/
│  │  ├─ ruby/
│  │  └─ php/
│  ├─ docker/             # Dockerfiles – one per language image
│  │  ├─ csharp/
│  │  ├─ node/
│  │  ├─ go/
│  │  ├─ ruby/
│  │  └─ php/
│  ├─ imageflow-dotnet/   # authoritative, hand‑maintained binding
│  ├─ imageflow-csharp/   # generated + custom shims (new)
│  ├─ imageflow-node/     # generated + custom shims
│  ├─ imageflow-go/       # generated + custom shims
│  ├─ imageflow-ruby/     # generated + custom shims
│  └─ imageflow-php/      # generated + custom shims
├─ imageflow_core/src/json/endpoints/
│  ├─ openapi_schema_v1.json      # Source of truth (generated when schema-export feature enabled)
│  └─ openapi_schema_v1.json.hash # Hash for change detection
├─ Cargo.toml             # workspace root; crates like imageflow_core, imageflow_abi
└─ .github/workflows/     # GitHub Actions YAML
```

*Each binding directory is a **stand‑alone publishable package**: it owns its own `VERSION` file / manifest.*

---

## 2 Schema Generation Architecture

The OpenAPI schema is **not** extracted at runtime. Instead, it's generated during the testing process when the `schema-export` feature is enabled and the schema.rs test is run:

### 2.1 Schema Source Location
- **Source**: `imageflow_core/src/json/endpoints/openapi_schema_v1.json`
- **Hash file**: `imageflow_core/src/json/endpoints/openapi_schema_v1.json.hash`
- **Feature flag**: `schema-export` (enabled by default in `imageflow_tool`)

### 2.2 Schema Generation Workflow
1. **Build with feature**: `cargo test --features schema-export --test schema`
2. **Schema generation**: The build process generates/updates `openapi_schema_v1.json`
3. **Change detection**: Compare hash to detect if schema changed
4. **PR creation**: If schema changed, create PR to update the file
5. **Binding regeneration**: After schema PR is merged, trigger binding generation

### 2.3 Schema Update Job
See `.github/workflows/schema-update.yml` for the complete workflow implementation.

---

## 3 Binding Generation Workflow

### 3.1 Trigger Conditions
Bindings are regenerated when:
- Schema PR is merged (via `workflow_run` trigger)
- Manual dispatch
- Changes to `bindings/templates/**` or `bindings/docker/**`

### 3.2 Reusable Workflow
See `.github/workflows/gen-binding.yml` for the complete reusable workflow implementation.

### 3.3 Dispatcher Matrix
See `.github/workflows/drive-bindings.yml` for the complete dispatcher workflow implementation.

---

## 4 Docker Images (Linux builds)

*One image per language*, stored at `ghcr.io/imageflow/bindings‑<lang>:<tag>`.

### 4.1 Image Contents
- Tool‑chain for the language
- `openapi-generator-cli.jar`
- Runtime and linters
- Pre‑installed native `libimageflow`
- Custom generation scripts

### 4.2 Example Dockerfile (C#)

```dockerfile
FROM mcr.microsoft.com/dotnet/sdk:8.0 AS build

# Install OpenAPI generator
RUN curl -L https://repo1.maven.org/maven2/org/openapitools/openapi-generator-cli/7.4.0/openapi-generator-cli-7.15.0.jar -o /usr/local/bin/openapi-generator-cli.jar

# Install additional tools
RUN apt-get update && apt-get install -y \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy generation scripts
COPY bindings/templates/csharp/generate.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/generate.sh

# Native libs are injected later by the CI artifact
COPY libimageflow/*.so /usr/local/lib/

WORKDIR /workspace
ENTRYPOINT ["/usr/local/bin/generate.sh"]
```

---

## 5 Language-Specific Shims

| Lang   | Transport hook                 | File                                   | Notes                                                            |
| ------ | ------------------------------ | -------------------------------------- | ---------------------------------------------------------------- |
| C#     | `SafeHandle` + `NativeLibrary` | `transport.cs`                         | Generated models + custom transport layer.                       |
| Node   | `node-ffi-napi`                | `transport.js`                         | Generated models + FFI transport.                                |
| Go     | `cgo` + `http.RoundTripper`    | `transport.go`                         | Generated models + custom transport.                             |
| Ruby   | `Fiddle`                       | `transport.rb`                         | Generated models + Fiddle transport.                             |
| PHP    | `FFI` ext                      | `transport.php`                        | Generated models + FFI transport.                                |

These files live under `bindings/<lang>/src/` and are copied verbatim after model generation.

---

## 6 Cross-Platform Constraints

* **Windows** – ensure `imageflow.dll` is in the same folder as the test runner or on `PATH`.
* **macOS** – notarisation is not required for CI; `DYLD_LIBRARY_PATH` suffices.
* **arm64** – GitHub offers native `linux‑arm64` and `windows‑arm64` runners; enable once the Rust core cross‑compiles.

---

## 7 PR Automation Strategy

### 7.1 Schema PRs
- **Trigger**: Schema file changes detected
- **Action**: Create single PR to update schema
- **Merge**: Auto-merge enabled for schema updates
- **Result**: Triggers binding generation workflow

### 7.2 Binding PRs
- **Trigger**: Schema PR merge or template changes
- **Action**: Create/update PR for each language
- **Strategy**: Replace existing PRs (don't accumulate)
- **Merge**: Manual review for binding changes

### 7.3 PR Management Script
See `scripts/manage-binding-prs.sh` for the complete PR management implementation.

---

## 8 Semantic Versioning per Binding

1. **Detect API delta** using [`openapi-diff`](https://github.com/OpenAPITools/openapi-diff):

   ```bash
   openapi-diff previous.json current.json --fail-on BREAKING
   ```
2. **Bump** `bindings/<lang>/VERSION` accordingly (`major` if breaking, else `minor`/`patch`).
3. Commit, tag `vX.Y.Z-<lang>`, and push via `gh release create`.

*`autorelease.sh` encapsulates this logic and is invoked in the reusable workflow.*

---

## 9 Integration with Existing CI

### 9.1 Artifact Reuse
The binding generation workflow reuses artifacts from the main `ci.yml` workflow:
- **Native binaries**: Downloaded from `native-binaries-*` artifacts
- **Schema**: Uses the committed `openapi_schema_v1.json` file
- **Version**: Parsed from the same version parsing action

### 9.2 Workflow Dependencies
```yaml
# In gen-binding.yml
jobs:
  generate:
    needs: []  # No direct dependency - uses artifacts
    runs-on: ubuntu-latest
```

### 9.3 Failure Handling
- **Individual binding failures** don't break the pipeline
- **Schema generation failures** create issues but don't block releases
- **Partial success** is acceptable - merge working bindings

---

## 10 Future Work

* **Schema vendor extensions** (`x-imageflow-node-type`, `x-imageflow-inputs`) → eliminate manual templates.
* **Split debug vs release Docker images** to shrink CI download size.
* **Cargo features** flag matrix to include optional codecs (HEIF, AVIF, AV1) and drive binding tests accordingly.
* **Advanced PR management** with dependency tracking between bindings.

---

© 2025 Imageflow Project. Pull requests welcome.
