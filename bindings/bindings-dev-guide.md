We need to create a complete guide to writing bindings for Imageflow - for both the ABI, for JSON serialization, and for exposing a fluent API to end users that handles a range of IO adapters. The ABI header is imageflow_default.h @imageflow_default.h, the json docs are in docs/src/ and the querystring (we want to offer an API for building that too) are in docs/querystring. 

The authoritative and maintained bindings are for .NET, located in bindings/imageflow-dotnet (sometimes, if we checked that out). Put this guide in bindings/bindings-dev-guide.md

Fully analyze both the json docs and the .NET bindings and fluent API, and pay special attention to both graph construction, io, and io_ids. Make a detailed guide to writing bindings that is useful for any language. It's ok to make notes about how something might be expressed in Ruby, PHP, Typescript, and Go. 

@/json @/querystring @/Fluent @/Bindings 


# A Developer's Guide to Writing Imageflow Bindings

Imageflow's power lies in its high-performance Rust core, but its reach comes from its ability to be integrated into many different programming languages. This is achieved through a stable C Application Binary Interface (ABI) that communicates using JSON.

This guide will walk you through the process of creating your own Imageflow bindings. We'll cover everything from the low-level C ABI to designing a high-level, user-friendly fluent API. Throughout this guide, we will reference the official [.NET bindings](https://github.com/imazen/imageflow/tree/main/bindings/imageflow-dotnet) as a canonical example of best practices.

## Table of Contents
1. [Imageflow Architecture Overview](#imageflow-architecture-overview)
2. [The Low-Level C ABI](#part-1-the-low-level-c-abi)
3. [The JSON Graph API](#part-2-the-json-graph-api)
4. [Designing a Fluent API](#part-3-designing-a-fluent-api)
5. [Integrating the Querystring API](#part-4-integrating-the-querystring-api)
6. [Schema-Driven Development](#part-5-schema-driven-development)
7. [Language-Specific Considerations](#language-specific-considerations)
8. [Integration with CI/CD](#integration-with-cicd)

---

## Imageflow Architecture Overview

Imageflow has a layered architecture:

1.  **The Core (`libimageflow`)**: A native library written in Rust, compiled with a C-compatible ABI. It contains all the image processing logic, decoding, and encoding capabilities.
2.  **The C ABI (`bindings/headers/imageflow_default.h`)**: A stable, C-style header that defines the functions, structs, and enums needed to interact with the core library from other languages.
3.  **The JSON API**: The "language" used to tell the core what to do. You construct a JSON object that describes a graph of operations (decode, resize, watermark, encode, etc.) and send it to the core via a C ABI function.
4.  **The OpenAPI Schema**: A machine-readable description of the JSON API, generated during the build process and stored as `imageflow_core/src/json/endpoints/openapi_schema_v1.json`.
5.  **Language Bindings (You are here!)**: A library in a specific language (e.g., C#, Go, TypeScript) that makes it easy for developers to use Imageflow. A good binding hides the complexity of both the C ABI and the JSON graph construction, providing a safe, idiomatic, and user-friendly interface.

---

## Part 1: The Low-Level C ABI

All interactions with `libimageflow` happen through the functions defined in `imageflow_default.h`. Your first step is to create a low-level wrapper around these functions.

### Linking and Loading

Your binding needs to load the native `libimageflow` library (`.dll`, `.so`, or `.dylib`). You can link it at compile time or, more flexibly, load it at runtime. The .NET bindings use a `NativeLibraryLoader` class that searches in common locations for the binary, which is a robust approach for distribution.

**First Call: ABI Compatibility Check**
Before calling any other function, you MUST verify that your binding's ABI version is compatible with the loaded library:

```c
// From imageflow_default.h
#define IMAGEFLOW_ABI_VER_MAJOR 3
#define IMAGEFLOW_ABI_VER_MINOR 0

bool compatible = imageflow_abi_compatible(IMAGEFLOW_ABI_VER_MAJOR, IMAGEFLOW_ABI_VER_MINOR);
if (!compatible) {
    // Handle error: versions are mismatched.
}
```

This prevents crashes and undefined behavior when a user tries to use your binding with an incompatible version of Imageflow.

### The `imageflow_context`

The `imageflow_context` is the central object for every Imageflow job. It tracks error states, memory allocations, and performance data for a single set of operations.

*   **Creation**: `struct imageflow_context *ctx = imageflow_context_create(MAJOR, MINOR);`
*   **Destruction**: `imageflow_context_destroy(ctx);`
*   **Thread Safety**: A context is **NOT thread-safe**. You must use one context per concurrent job/thread. Do not share contexts across threads.

**Best Practice**: In a language with automatic memory management (like C#, Java, Go, etc.), wrap the `imageflow_context*` in a class that handles its creation and destruction automatically (e.g., using a finalizer or a `SafeHandle` like .NET's `JobContextHandle`). This prevents memory leaks.

### Error Handling

Imageflow uses the context to report errors. It does not use return codes for most functions.

1.  **Check for Errors**: After a function call, check if an error occurred with `bool error_exists = imageflow_context_has_error(ctx);`.
2.  **Get Error Message**: If an error exists, retrieve a detailed, human-readable message with `imageflow_context_error_write_to_buffer()`. You provide a buffer, and it writes the UTF-8 error message into it. This function returns `false` if the provided buffer was too small to hold the entire message.
3.  **Get Error Codes**: You can also get a coarse error category as an HTTP status code (`imageflow_context_error_as_http_code()`) or a POSIX exit code (`imageflow_context_error_as_exit_code()`), which can be useful for programmatic error handling.

**Best Practice**: Create a custom exception type (e.g., `ImageflowException`) and a factory method that populates it by calling the error functions on the context. This provides a clear and idiomatic error handling mechanism for users of your binding.

**Best Practice for Buffer Sizing**: When calling `imageflow_context_error_write_to_buffer()`, the error message might be larger than your initial buffer. The function signals this by returning `false`. The recommended approach, as seen in the .NET bindings, is to use a loop: start with a reasonably sized buffer (e.g., 2KB), and if the call fails because the buffer is too small, double the buffer size and try again. It is wise to cap this at a maximum size (e.g., 32KB) to prevent allocating excessive memory for an unexpectedly long error message.

### Managing I/O and Memory

You provide inputs and define outputs through the context *before* executing the job. Each input and output is identified by a 32-bit integer, the `io_id`.

**Inputs**

Use `imageflow_context_add_input_buffer()` to provide source data (e.g., a JPEG file's contents).

```c
bool success = imageflow_context_add_input_buffer(ctx,
                                                  io_id,         // A unique integer, e.g., 0
                                                  buffer_ptr,    // Pointer to the data
                                                  buffer_len,    // Length of the data
                                                  lifetime);     // Crucial memory lifetime hint
```

The `lifetime` parameter is critical for performance and safety:
*   `imageflow_lifetime_lifetime_outlives_function_call`: Imageflow will immediately make its own copy of the data. This is safer but involves a memory copy.
*   `imageflow_lifetime_lifetime_outlives_context`: You promise that the `buffer_ptr` will remain valid and unchanged until `imageflow_context_destroy()` is called. This avoids a copy but requires careful memory management, especially in garbage-collected languages.

**Best Practice**: For `outlives_context`, you must "pin" the memory so the garbage collector can't move it. .NET achieves this with `Memory<T>.Pin()` which returns a `MemoryHandle`. Your binding should manage these pinned handles and unpin them when the context is destroyed.

**Outputs**

Use `imageflow_context_add_output_buffer()` to prepare a destination.

```c
bool success = imageflow_context_add_output_buffer(ctx, io_id); // A unique integer, e.g., 1
```

Imageflow will allocate a buffer for this output. You retrieve a pointer to it after the job is done.

### Executing Jobs & Retrieving Results

1.  **Send JSON**: The primary execution function is `imageflow_context_send_json()`. It takes the context, an endpoint name (almost always `"v1/build"`), and the JSON job description as a UTF-8 byte buffer.
2.  **Get Response Handle**: It returns a `const struct imageflow_json_response*`. This is an opaque handle, not the JSON string itself.
3.  **Read Response**: Pass this handle to `imageflow_json_response_read()` to get a pointer to the JSON response data (which contains metadata about the results) and the overall job status code.
4.  **Get Output Bytes**: After a successful job, call `imageflow_context_get_output_buffer_by_id()` with an output `io_id` to get a pointer and length for the resulting image data.
5.  **Cleanup**: The response handle must be freed with `imageflow_json_response_destroy()`, and the context with `imageflow_context_destroy()`.

**Best Practice**: Wrap the response handle in a `SafeHandle` (`JsonResponseHandle` in .NET) to ensure it's always cleaned up.

---

## Part 2: The JSON Graph API

This is the instruction set for Imageflow. A good binding will generate this JSON for the user. The full JSON API is documented in `docs/src/json/`.

### High-Level Structure

A typical job object has up to three top-level keys:

*   `"io"`: An array defining inputs and outputs when using `imageflow_tool` from the command line. When using the C ABI, this isn't strictly necessary since you add I/O via `imageflow_context_add_*_buffer`, but it's good practice to know it exists.
*   `"security"`: An object defining resource limits to prevent denial-of-service attacks from malicious images or requests (e.g., setting `max_decode_size` and `max_frame_size`). This is **essential** for any web-facing service.
*   `"framewise"`: An object that contains the actual processing commands, applied to each frame of an image.

### Simple `steps` vs. `graph` Execution

Inside `"framewise"`, you can specify either `"steps"` or a `"graph"`:

*   **`"steps"`**: A simple array of operations executed linearly. The output of one step is implicitly the input to the next. Great for simple, single-input, single-output jobs.
*   **`"graph"`**: For everything else. The graph model is more powerful and what a fluent API should target. It allows for multiple inputs (like for watermarks), multiple outputs (generating a thumbnail and a large version in one job), and complex processing chains.

### Anatomy of a Graph

A `"graph"` object has two keys:

*   **`"nodes"`**: An object mapping stringified node IDs (e.g., `"0"`, `"1"`, `"2"`) to operation objects.
*   **`"edges"`**: An array that defines the connections between nodes. Each edge specifies a `from` node ID, a `to` node ID, and a `kind` (`"input"` for primary data flow, `"canvas"` for compositing operations).

```json
{
  "graph": {
    "nodes": {
      "0": { "decode": { "io_id": 0 } },
      "1": { "constrain": { "w": 500 } },
      "2": { "encode": { "io_id": 1, "preset": "mozjpeg" } }
    },
    "edges": [
      { "from": 0, "to": 1, "kind": "input" },
      { "from": 1, "to": 2, "kind": "input" }
    ]
  }
}
```

This example decodes input `0`, constrains it to 500px wide, and encodes the result to output `1`.

### Key Operation Nodes

Your bindings should provide typed wrappers or builders for these common operations:

*   `"decode"`: The start of a graph. Specifies the `io_id` of an input buffer. Can accept `commands` for performance, like `jpeg_downscale_hints`.
*   `"encode"`: A terminal node. Specifies the `io_id` of an output buffer and an encoding `preset` like `"mozjpeg"`, `"pngquant"`, or `"webplossy"`.
*   `"constrain"`: The primary resizing/scaling tool. It has many options like `mode` (`within`, `crop`, `pad`), `w`, `h`, `gravity`, and `hints` for resampling quality.
*   `"watermark"`: Adds a watermark. This requires a graph, as it takes the main image as its `input` and the watermark image from another `decode` node (referenced by `io_id`).
*   `"create_canvas"`: Creates a new, blank image. Useful as a base for compositing.
*   `"draw_image_exact"`: A compositing node. It requires two inputs via edges: a main `canvas` and an `input` to be drawn onto it.
*   `"command_string"`: Executes a querystring-style command. A powerful tool for bridging APIs.

---

## Part 3: Designing a Fluent API

The goal of a fluent API is to hide the C ABI and JSON details, providing a safe, discoverable, and easy-to-use interface. The `Imageflow.Fluent` namespace in the .NET bindings is our reference.

### The Job Builder Pattern (`ImageJob`)

Create a central "job" or "builder" class. This class will be the main entry point for users.

*   It manages the list of inputs, outputs, and all the graph nodes created for the job.
*   It should automatically generate unique `io_id`s for the user.
*   It provides the starting methods for a graph, such as `Decode()` or `CreateCanvas()`.

### Chainable Nodes (`BuildNode`)

Each operation that can be followed by another should return a "node" object. This allows for method chaining.

```csharp
// C# Example
BuildNode node = job.Decode(myImageBytes)
                    .ConstrainWithin(800, 600)
                    .Rotate90();
```

Each `BuildNode` object should store:
1.  A reference back to the main `ImageJob` builder.
2.  A reference to its input `BuildNode`.
3.  The JSON data for its *own* operation (e.g., the `{ "rotate_90": null }` object).

### Abstracting I/O

Don't force users to deal with byte arrays. A good binding should accept common I/O primitives for its language.

*   **Inputs**: Create an interface (e.g., `IAsyncMemorySource` in .NET) and provide adapters for streams, file paths, and memory buffers. In Go, you might accept an `io.Reader`. In TypeScript, a `Buffer` or `ReadableStream`.
*   **Outputs**: Similarly, abstract the destination (e.g., `IOutputDestination` in .NET) with implementations that can write to a stream or an in-memory buffer.

### Assembling the JSON Graph

When the user is done building their chain and wants to execute it, the binding must convert the linked list of `BuildNode` objects into the final JSON graph.

1.  Traverse all the nodes created under the `ImageJob`.
2.  Assign a sequential ID to each unique node.
3.  Create the `"nodes"` object in the JSON by adding the JSON data from each `BuildNode` at its assigned ID.
4.  Create the `"edges"` array by looking at the `Input` and `Canvas` references on each `BuildNode` and adding the appropriate `from`/`to` edge.

### Handling Job Execution

The final step is to provide a way to execute the job. The .NET bindings use a `Finish()` method that returns a `FinishJobBuilder`, allowing final options like security settings or a cancellation token to be set.

This final object has the method that performs the work (e.g., `InProcessAsync()`):
1.  It triggers the JSON graph assembly.
2.  It resolves all the abstract input sources into pinned memory buffers.
3.  It calls the low-level C ABI functions in the correct order (`create_context`, `add_input_buffer`, `add_output_buffer`, `send_json`).
4.  It retrieves the output buffers via `get_output_buffer_by_id` and writes the data to the user-provided output destinations.
5.  It parses the JSON response into a structured result object (e.g., `BuildJobResult`).
6.  It cleans up the context and any other resources.

---

## Part 4: Integrating the Querystring API

Imageflow also supports a powerful querystring API for backwards compatibility with ImageResizer. Your binding can support this easily using the `"command_string"` JSON node.

Instead of building a full graph, you can construct a very simple JSON object that passes the querystring directly to Imageflow's optimized RIAPI parser.

```json
{
  "command_string": {
    "kind": "ir4",
    "value": "width=100&height=100&mode=max",
    "decode": 0,
    "encode": 1
  }
}
```

**Best Practice**: Provide a dedicated method in your fluent API, like `.BuildCommandString(source, dest, "w=100")`, that handles this for the user. It's a simple and effective way to expose the entire querystring feature set with minimal effort.

---

## Part 5: Schema-Driven Development

The modern approach to Imageflow binding development leverages the OpenAPI schema that's generated during the build process.

### Schema Source
- **Location**: `imageflow_core/src/json/endpoints/openapi_schema_v1.json`
- **Generation**: Built with `cargo build --features schema-export`
- **Version Control**: Committed to the repository as the source of truth

### Using the Schema for Model Generation

Instead of hand-writing data models, use the OpenAPI schema to generate them:

```bash
# Generate C# models
openapi-generator-cli generate \
  -i imageflow_core/src/json/endpoints/openapi_schema_v1.json \
  -g csharp \
  --global-property models \
  -o ./generated/

# Generate TypeScript models
openapi-generator-cli generate \
  -i imageflow_core/src/json/endpoints/openapi_schema_v1.json \
  -g typescript-axios \
  --global-property models \
  -o ./generated/
```

### Custom Transport Layer

After generating models, create a custom transport layer that:
1. Takes generated model objects
2. Serializes them to JSON
3. Calls the C FFI functions
4. Deserializes responses back to generated models

### Benefits of Schema-Driven Development
- **Type Safety**: Generated models are always in sync with the API
- **Documentation**: Schema includes descriptions for all fields
- **Maintainability**: API changes automatically propagate to models
- **Consistency**: All bindings use the same data structures

---

## Language-Specific Considerations

*   **Go**:
    *   **FFI**: Use `cgo` for C ABI interaction.
    *   **JSON**: Use structs and `encoding/json` for type-safe JSON serialization.
    *   **Errors**: Return `error` values from functions, which is idiomatic Go.
    *   **I/O**: Use `io.Reader` for inputs and `io.Writer` for outputs. Goroutines are a natural fit for async operations.
*   **TypeScript/Node.js**:
    *   **FFI**: Use a library like `node-ffi-napi` or write a native C++ addon.
    *   **JSON**: JSON is native. Use `interface`s for type-safety.
    *   **I/O**: Use `Buffer` and `ReadableStream`/`WritableStream` as the standard I/O primitives. `async/await` makes the API clean.
*   **Ruby**:
    *   **FFI**: The `fiddle` library in the standard library is excellent for C interop.
    *   **JSON**: Use the `json` gem. Hash-like objects are a natural fit for building JSON.
    *   **Fluent API**: Ruby's method chaining and use of blocks make for a very elegant fluent API (e.g., `node.branch { |b| b.encode(...) }`).
*   **PHP**:
    *   **FFI**: PHP has a built-in FFI extension that can be used to call C functions.
    *   **JSON**: `json_encode` and `json_decode` are standard. Use classes and associative arrays to construct the JSON structure.
    *   **I/O**: Work with PHP's stream resources.

---

## Integration with CI/CD

The binding development process integrates with the CI/CD pipeline:

### Schema Updates
1. **Automatic Detection**: CI detects when the schema has changed
2. **PR Creation**: Creates a PR to update the schema file
3. **Auto-Merge**: Schema updates are auto-merged. If possible with github, we cancel all workflow runs for this since it will re-run when we merge, and will start faster.
4. **Binding Trigger**: Schema merge triggers binding regeneration

### Binding Generation
1. **Parallel Generation**: Each language binding is generated in parallel
2. **PR Creation**: Each binding gets its own PR with generated updates
3. **PR Replacement**: New PRs replace old ones (no accumulation)
4. **Manual Review**: Binding changes require manual review before merge

This approach ensures that all bindings stay in sync with the core API while maintaining the flexibility to customize the transport layer and fluent API for each language's idioms.

@/json @/querystring @/Fluent @/Bindings 
