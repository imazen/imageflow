#!/bin/bash
set -e

echo "Generating OpenAPI schema..."

# Run imageflow_tool to export the schema to the mounted /output volume
imageflow_tool --export-openapi-schema /output/openapi.json

echo "Schema generation complete. Output saved to /output/openapi.json"

# TODO: Add steps for documentation generation (e.g., using redoc-cli)
# Example:
# echo "Generating Markdown documentation..."
# npm install -g @redocly/cli
# redocly build-docs /output/openapi.json -o /output/API.md
# echo "Documentation generation complete. Output saved to /output/API.md"

# TODO: Add steps for API analysis/suggestion 