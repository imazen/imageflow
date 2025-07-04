# A Guide to Automating Bindings with Imageflow's JSON Schemas

Manually creating a language binding can be a time-consuming and error-prone process. The JSON API for Imageflow is extensive, with dozens of commands and complex data structures. Keeping a handwritten binding in sync with `libimageflow`'s features is a significant maintenance burden.

This guide outlines a more advanced, schema-driven approach to creating and maintaining bindings. By using `libimageflow` itself as the "source of truth", we can automate the generation of data models, documentation, and even parts of a fluent API.

## Table of Contents
1. [Schema-First Development](#the-core-concept-schema-first-development)
2. [Generating Data Models](#part-1-generating-data-models-with-openapi)
3. [Generating Documentation](#part-2-generating-documentation)
4. [Generating Fluent APIs](#part-3-generating-a-fluent-api---the-challenge)
5. [Fully Automated Bindings](#part-4-the-holy-grail---a-fully-automated-binding)
6. [CI/CD Integration](#part-5-integration-with-cicd)

---

## The Core Concept: Schema-First Development

Imageflow's JSON API schema is **not** extracted at runtime. Instead, it's generated during the build process when the `schema-export` feature is enabled and stored as a committed file in the repository.

### Schema Source Location
- **Source**: `imageflow_core/src/json/endpoints/openapi_schema_v1.json`
- **Hash file**: `imageflow_core/src/json/endpoints/openapi_schema_v1.json.hash`
- **Feature flag**: `schema-export` (enabled by default in `imageflow_tool`)

### Schema Generation Process
1. **Build with feature**: `cargo build --features schema-export`
2. **Schema generation**: The build process generates/updates `openapi_schema_v1.json`
3. **Change detection**: Compare hash to detect if schema changed
4. **PR creation**: If schema changed, create PR to update the file
5. **Binding regeneration**: After schema PR is merged, trigger binding generation

This approach ensures that:
- The schema is always version-controlled and traceable
- Changes to the API are immediately reflected in the schema
- Binding generation is triggered automatically when the API changes
- The schema serves as the single source of truth for all bindings

---

## Part 1: Generating Data Models with OpenAPI

The most significant benefit of the schema-first approach is automatic model generation. You can create all the classes, structs, or interfaces needed for your binding without writing them by hand.

### The Workflow

1. **Use the Committed Schema**:
   Your binding's build script uses the committed schema file:
   ```bash
   # The schema is already available in the repository
   SCHEMA_PATH="imageflow_core/src/json/endpoints/openapi_schema_v1.json"
   ```

2. **Use an OpenAPI Generator**:
   Use a standard tool like [openapi-generator-cli](https://openapi-generator.tech/) to generate code from the schema file. You can target dozens of languages.

   For example, to generate TypeScript models:
   ```bash
   openapi-generator-cli generate -i $SCHEMA_PATH -g typescript-axios -o ./generated/
   ```

   To generate models for Go:
   ```bash
   openapi-generator-cli generate -i $SCHEMA_PATH -g go -o ./generated/
   ```

### What You Get

This process will automatically create typed data structures for all of Imageflow's JSON objects, such as:
*   `Build001`, `Execute001`, etc. (The top-level job objects)
*   `Constraint`, `Watermark`, `EncoderPreset`, `ColorSrgb` (Every command and its parameters)
*   `JobResult`, `ImageInfo` (The response objects)

By doing this, you've saved yourself hours of tedious work and eliminated a major source of bugs. Your binding's data layer will always be perfectly in sync with the `libimageflow` version you're building against.

### Adapting Generated Code for FFI (The Important Part!)

You will immediately notice that OpenAPI generators produce code for making **HTTP requests**. They will create a client that tries to `POST` to `/v1/build`. This code is not directly usable, as Imageflow bindings interact with a local library via a C Foreign Function Interface (FFI), not a network socket.

The strategy is to **use the generated models, but replace the generated HTTP client with your own FFI-based transport layer.**

The workflow looks like this:

1.  **Generate Code**: Run your OpenAPI generator as described above.
2.  **Keep the Models**: The generated code will likely be split into `models` and `api` (or `client`) directories. Keep the `models` directory. These are the language-native representations of Imageflow's JSON objects, and they are perfectly reusable.
3.  **Discard the API/Client**: Delete or ignore the generated `api` or `client` directory. You will not be making HTTP calls.
4.  **Implement a Custom Transport**: This is the core of your binding's execution logic. Your custom "client" or "job executor" will:
    a. Accept the generated model objects (e.g., a `Build001` object) from the user or a fluent API.
    b. Serialize the model object into a UTF-8 JSON byte buffer.
    c. Pass this byte buffer to the C FFI function `imageflow_context_send_json`, using the appropriate endpoint name (e.g., `"v1/build"`).
    d. Receive the JSON response from Imageflow.
    e. Deserialize the response JSON into the appropriate **generated response model** (e.g., `JobResult` or `ImageInfo`).
    f. Return this structured result to the calling code.

Many OpenAPI generators allow you to generate only the models, which simplifies this process. For example, you can often pass a flag like `--global-property=models`.

By separating the data models from the transport mechanism, you get the best of both worlds: fully automated, type-safe data structures, and a purpose-built execution engine that correctly interfaces with the C ABI.

---

## Part 2: Generating Documentation

The OpenAPI schema is also rich with documentation. Every field, object, and endpoint has a `description` property containing Markdown-formatted text.

Your build process can parse the `openapi_schema_v1.json` file and extract these descriptions to generate documentation for your binding. You can:
*   Create Markdown files for a documentation website.
*   Insert the descriptions as inline code comments (e.g., XML-docs in C#, GoDoc in Go, JSDoc in TypeScript) in the generated models.

This ensures that your binding's documentation is always accurate and reflects the capabilities of the underlying native library.

---

## Part 3: Generating a Fluent API - The Challenge

Generating a rich, chainable fluent API (like the one in the .NET bindings) is more complex than just generating data models. Standard OpenAPI generators are designed for simple request/response clients and do not understand Imageflow's graph-based execution model. A generated HTTP client is linear; a fluent Imageflow binding is a graph builder.

To bridge this gap, your generator needs to understand Imageflow's semantics:
1.  **Graph Structure**: It must know that operations are nodes in a graph connected by edges.
2.  **Node Inputs**: It must identify which operations are "source" nodes that start a chain (`decode`, `create_canvas`), which are "filters" that modify an input (`constrain`, `rotate_90`), and which are "terminal" nodes that finish a chain (`encode`).
3.  **Special Inputs**: It must understand nodes that take more than one image input, like `draw_image_exact`, which has a primary `input` and a `canvas` input.

---

## Part 4: The Holy Grail - A Fully Automated Binding

Achieving one-step binding generation requires a custom code generator tailored to Imageflow. This tool would read the `openapi_schema_v1.json` schema and use a set of custom templates to write the entire binding, including the models, the FFI transport layer, and the fluent API.

### The Custom Generator Workflow

A fully automated build script would look like this:

1.  **Use the Schema**:
    ```bash
    SCHEMA_PATH="imageflow_core/src/json/endpoints/openapi_schema_v1.json"
    ```
2.  **Run the Custom Generator**:
    ```bash
    # A hypothetical custom tool
    imageflow-binding-generator --input $SCHEMA_PATH --language csharp --templates ./bindings/templates/csharp/ --output ./bindings/imageflow-csharp/Generated/
    ```
This process would regenerate everything, ensuring the binding is always perfectly in sync.

### Anatomy of a Custom Generator

Your custom generator would need three main components:

1.  **Schema Interpreter**: A robust parser for the `openapi_schema_v1.json` file. It wouldn't just read the schema; it would need to *interpret* it based on Imageflow conventions to identify which JSON objects represent graph nodes.

2.  **Templating Engine**: Use a standard templating language like [Scriban](https://github.com/scriban/scriban) (for .NET), [Handlebars](https://handlebarsjs.com/), or Go's built-in `text/template`. You would create a set of templates for your target language.

3.  **A Set of Custom Templates**:
    *   **Model Templates**: Similar to what standard generators use, these would create the data classes for all schema components.
    *   **FFI Transport Template**: A template for the code that calls the C FFI functions (`imageflow_context_send_json`). The generator would fill this in with the correct endpoint names and model types based on the schema.
    *   **Fluent API Templates**: This is the most complex part. You would create templates for a `Job` class and a `BuildNode` class. The generator would iterate through all the JSON objects that represent graph nodes and generate a corresponding fluent method for each one (e.g., a `.constrain()` method that takes a `Constraint` model and returns a new `BuildNode`).

### Enriching the Schema with Semantics

For a custom generator to work effectively, the schema needs to contain hints about the graph structure. While not yet present in Imageflow's schema, this could be achieved in the future using OpenAPI vendor extensions (keys that start with `x-`).

For example, a `constrain` operation in the schema could be augmented like this:
```json
"constrain": {
    "x-imageflow-node-type": "filter",
    "type": "object",
    "properties": { ... }
}
```
*   `x-imageflow-node-type`: Could be `'source'`, `'filter'`, or `'terminal'`.
*   `x-imageflow-inputs`: Could list the expected input kinds, like `['input', 'canvas']`.

With these hints, a custom generator has all the information it needs to build a complete, correct, and fully-featured fluent API automatically. While building this generator is a significant upfront investment, it offers a path to nearly effortless maintenance and rapid development of bindings for new languages.

---

## Part 5: Integration with CI/CD

The schema-first approach integrates seamlessly with continuous integration:

### Schema Update Workflow
1. **Automatic Detection**: CI detects when the schema has changed
2. **PR Creation**: Creates a PR to update the schema file
3. **Auto-Merge**: Schema updates are auto-merged (they're generated, not hand-written)
4. **Binding Trigger**: Schema merge triggers binding regeneration

### Binding Generation Workflow
1. **Parallel Generation**: Each language binding is generated in parallel
2. **PR Creation**: Each binding gets its own PR with generated updates
3. **PR Replacement**: New PRs replace old ones (no accumulation)
4. **Manual Review**: Binding changes require manual review before merge

### Benefits of This Approach
- **Consistency**: All bindings use the same schema version
- **Automation**: Minimal manual intervention required
- **Traceability**: Every binding change is traceable to a schema change
- **Reliability**: Generated code is less error-prone than hand-written code
- **Maintainability**: Changes to the API automatically propagate to all bindings

This approach transforms binding maintenance from a manual, error-prone process into an automated, reliable system that scales with the number of supported languages. 
