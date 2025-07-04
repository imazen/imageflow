# OpenAPI Generator Guide for Imageflow

This guide provides a focused overview of the OpenAPI Generator CLI, specifically as it relates to the Imageflow project. For complete documentation, always refer to the [official OpenAPI Generator website](https://openapi-generator.tech/).

## Command-Line Usage

The `generate` command is the workhorse of the generator toolset. It has many options available.

```text
openapi-generator-cli help generate

NAME
       openapi-generator-cli generate - Generate code with the specified generator.

SYNOPSIS
       openapi-generator-cli generate
              [(-a <authorization> | --auth <authorization>)]
              [--api-name-suffix <api name suffix>]
              [--api-package <api package>]
              [--artifact-id <artifact id>]
              [--artifact-version <artifact version>]
              [(-c <configuration file> | --config <configuration file>)]
              [--dry-run]
              [(-e <templating engine> | --engine <templating engine>)]
              [--enable-post-process-file]
              [(-g <generator name> | --generator-name <generator name>)]
              [--generate-alias-as-model]
              [--git-host <git host>]
              [--git-repo-id <git repo id>]
              [--git-user-id <git user id>]
              [--global-property <global properties>...]
              [--group-id <group id>]
              [--http-user-agent <http user agent>]
              [(-i <spec file> | --input-spec <spec file>)]
              [--ignore-file-override <ignore file override location>]
              [--import-mappings <import mappings>...]
              [--instantiation-types <instantiation types>...]
              [--invoker-package <invoker package>]
              [--language-specific-primitives <language specific primitives>...]
              [--legacy-discriminator-behavior]
              [--library <library>]
              [--log-to-stderr]
              [--minimal-update]
              [--model-name-prefix <model name prefix>]
              [--model-name-suffix <model name suffix>]
              [--model-package <model package>]
              [(-o <output directory> | --output <output directory>)]
              [(-p <additional properties> | --additional-properties <additional properties>)...]
              [--package-name <package name>]
              [--release-note <release note>]
              [--remove-operation-id-prefix]
              [--reserved-words-mappings <reserved word mappings>...]
              [(-s | --skip-overwrite)]
              [--server-variables <server variables>...]
              [--skip-operation-example]
              [--skip-validate-spec]
              [--strict-spec <true/false strict behavior>]
              [(-t <template directory> | --template-dir <template directory>)]
              [--type-mappings <type mappings>...]
              [(-v | --verbose)]
```

## Customization

The OpenAPI Generator is highly customizable. While we strive to use the default generators with minimal configuration, it's useful to know how to extend them.

### Overriding Templates

The most common customization is to override the built-in templates. You can provide your own templates using the `--template-dir` (or `-t`) option. The generator will look for templates in this directory first before falling back to the built-in ones.

For more details, see the official documentation on [templating](https://openapi-generator.tech/docs/templating).

### User-defined Templates & Supporting Files

You can also provide additional supporting files and extensions to built-in templates via an external configuration file (`-c` or `--config`).

For example, you can define custom files in your `config.yaml`:

```yaml
templateDir: my_custom_templates
additionalProperties:
  # ... other properties
files:
  AUTHORS.md: {}
  api_interfaces.mustache:
    templateType: API
    destinationFilename: Impl.kt
  other/check.mustache:
    destinationFilename: scripts/check.sh
```

This allows you to add static files (like `AUTHORS.md`) or define new template types that generate additional files alongside the standard ones.

The available `templateType` options are:
* `API`
* `APIDocs`
* `APITests`
* `Model`
* `ModelDocs`
* `ModelTests`
* `SupportingFiles` (default)

For more details, see the official documentation on [customization](https://openapi-generator.tech/docs/customization).