# Imageflow API Schema Generation

This directory contains the tools and configuration for generating the OpenAPI schema and related documentation for the Imageflow JSON API.

## Goals

1.  **Generate OpenAPI Schema:** Automatically generate an `openapi.json` file based on the API definitions in `imageflow_core`.
2.  **Generate Markdown Docs:** Convert the `openapi.json` schema into human-readable Markdown documentation (`API.md`).
3.  **Suggest Fluent API:** (Future Goal) Analyze the OpenAPI schema to suggest potential structures for fluent API wrappers in different languages.
4.  **Upload Schema (Optional):** Provide a mechanism (e.g., in a GitHub Workflow) to upload the generated `openapi.json` to a storage location like S3.

## Steps (Manual / Local Testing via Docker)

1.  **Build the Docker Image:**
    ```bash
    docker build -t imageflow-schema-gen -f schema/Dockerfile .
    ```
2.  **Run the Generation Script:**
    ```bash
    docker run --rm -v ./schema:/output imageflow-schema-gen
    ```
    This will:
    *   Build `imageflow_tool` with the `schema-export` feature.
    *   Run `imageflow_tool --export-openapi-schema /output/openapi.json`.
    *   (Future) Run documentation generation tools (e.g., Redocly CLI) using `/output/openapi.json` to create `/output/API.md`.
    *   (Future) Run API analysis tools.
    *   The generated `openapi.json` and `API.md` will appear in the local `schema/` directory.

## GitHub Workflow

A GitHub Actions workflow (`.github/workflows/generate-schema.yml`) automates this process, typically running on pushes to the main branch or manually triggered. It performs the steps outlined above and can optionally upload the resulting schema to S3 or another artifact store. 