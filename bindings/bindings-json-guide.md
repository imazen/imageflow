# A Guide to Automating Bindings with Imageflow's JSON Schemas

Manually creating a language binding can be a time-consuming and error-prone process. The JSON API for Imageflow is extensive, with dozens of commands and complex data structures. Keeping a handwritten binding in sync with `libimageflow`'s features is a significant maintenance burden.

This guide outlines a more advanced, schema-driven approach to creating and maintaining bindings. By using `libimageflow` itself as the "source of truth", we can automate the generation of data models, documentation, and even parts of a fluent API.

## The Core Concept: Self-Describing API

Imageflow's JSON API is self-describing. When you have a build of `libimageflow` (or the `imageflow_tool`), you can ask it for the schemas of the APIs it supports. This is done by calling special "meta" endpoints.

The primary tool for this is `imageflow_tool`. You can use it in build scripts to call these endpoints and save their output.

### Key Meta-Endpoints

You can see a list of available endpoints by running:
`imageflow_tool v1/schema/list-schema-endpoints`

The most important ones for automation are:
*   `v1/schema/openapi/latest/get`: Returns a full **OpenAPI v3 schema** for the entire JSON API. This is the cornerstone of automation.
*   `v1/schema/riapi/latest/get`: Returns a detailed JSON schema for the querystring API (RIAPI).
*   `v1/schema/json/latest/v1/all`: Returns granular JSON schemas for the inputs and outputs of each individual endpoint.

## Part 1: Generating Data Models with OpenAPI

The most significant benefit of the schema-first approach is automatic model generation. You can create all the classes, structs, or interfaces needed for your binding without writing them by hand.

### The Workflow

1.  **Export the OpenAPI Schema**:
    In your binding's build script, start by exporting the live schema from `imageflow_tool`:
    ```bash
    imageflow_tool v1/schema/openapi/latest/get > openapi.json
    ```
    This command creates an `openapi.json` file that contains a complete definition of every JSON object, field, and type used in the Imageflow API.

2.  **Use an OpenAPI Generator**:
    Use a standard tool like [openapi-generator-cli](https://openapi-generator.tech/) to generate code from the schema file. You can target dozens of languages.

    For example, to generate TypeScript models:
    ```bash
    openapi-generator-cli generate -i openapi.json -g typescript-axios -o ./generated/
    ```

    To generate models for Go:
    ```bash
    openapi-generator-cli generate -i openapi.json -g go -o ./generated/
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

## Part 2: Generating Documentation

The OpenAPI schema is also rich with documentation. Every field, object, and endpoint has a `description` property containing Markdown-formatted text.

Your build process can parse the `openapi.json` file and extract these descriptions to generate documentation for your binding. You can:
*   Create Markdown files for a documentation website.
*   Insert the descriptions as inline code comments (e.g., XML-docs in C#, GoDoc in Go, JSDoc in TypeScript) in the generated models.

This ensures that your binding's documentation is always accurate and reflects the capabilities of the underlying native library.

## Part 3: Generating a Fluent API - The Challenge

Generating a rich, chainable fluent API (like the one in the .NET bindings) is more complex than just generating data models. Standard OpenAPI generators are designed for simple request/response clients and do not understand Imageflow's graph-based execution model. A generated HTTP client is linear; a fluent Imageflow binding is a graph builder.

To bridge this gap, your generator needs to understand Imageflow's semantics:
1.  **Graph Structure**: It must know that operations are nodes in a graph connected by edges.
2.  **Node Inputs**: It must identify which operations are "source" nodes that start a chain (`decode`, `create_canvas`), which are "filters" that modify an input (`constrain`, `rotate_90`), and which are "terminal" nodes that finish a chain (`encode`).
3.  **Special Inputs**: It must understand nodes that take more than one image input, like `draw_image_exact`, which has a primary `input` and a `canvas` input.

## Part 4: The Holy Grail - A Fully Automated Binding

Achieving one-step binding generation requires a custom code generator tailored to Imageflow. This tool would read the `openapi.json` schema and use a set of custom templates to write the entire binding, including the models, the FFI transport layer, and the fluent API.

### The Custom Generator Workflow

A fully automated build script would look like this:

1.  **Export the Schema**:
    ```bash
    imageflow_tool v1/schema/openapi/latest/get > openapi.json
    ```
2.  **Run the Custom Generator**:
    ```bash
    # A hypothetical custom tool
    imageflow-binding-generator --input openapi.json --language csharp --templates ./codegen/csharp/ --output ./src/Imageflow.Fluent/Generated/
    ```
This process would regenerate everything, ensuring the binding is always perfectly in sync.

### Anatomy of a Custom Generator

Your custom generator would need three main components:

1.  **Schema Interpreter**: A robust parser for the `openapi.json` file. It wouldn't just read the schema; it would need to *interpret* it based on Imageflow conventions to identify which JSON objects represent graph nodes.

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
