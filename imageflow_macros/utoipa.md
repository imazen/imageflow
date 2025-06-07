
Okay, let's break down how to programmatically construct and modify `utoipa::openapi::OpenApi` objects without relying solely on the derive macros for paths and operations.

### 1. Core Structures

Here are the key structs involved in manually building an OpenAPI definition with `utoipa`:

*   **`utoipa::openapi::OpenApi`**: The root object of the OpenAPI specification document.
    *   `openapi`: `String` - The OpenAPI version (e.g., "3.1.0").
    *   `info`: `utoipa::openapi::Info` - Metadata about the API (title, version, description, etc.).
    *   `servers`: `Option<Vec<utoipa::openapi::Server>>` - A list of server URLs for the API.
    *   `paths`: `utoipa::openapi::Paths` - Holds the available paths and operations for the API. **This is the primary field you'll modify.**
    *   `components`: `Option<utoipa::openapi::Components>` - Holds reusable schemas, responses, parameters, etc. Schemas derived via `ToSchema` are typically placed here.
    *   `security`: `Option<Vec<utoipa::openapi::security::SecurityRequirement>>` - Security scheme requirements.
    *   `tags`: `Option<Vec<utoipa::openapi::Tag>>` - A list of tags used by operations, with optional descriptions.
    *   `external_docs`: `Option<utoipa::openapi::ExternalDocs>` - Link to external documentation.

*   **`utoipa::openapi::Paths`**: A map containing the API paths and their definitions.
    *   `paths`: `indexmap::IndexMap<String, utoipa::openapi::PathItem>` - The core map where keys are the path strings (e.g., `/v1/build`) and values are `PathItem` objects describing the operations available at that path. You access this map directly to add or modify paths.
    *   `extensions`: `Option<indexmap::IndexMap<String, serde_json::Value>>` - Allows for specification extensions.

*   **`utoipa::openapi::PathItem`**: Describes the operations available on a single path.
    *   `get`, `put`, `post`, `delete`, `options`, `head`, `patch`, `trace`: `Option<utoipa::openapi::Operation>` - Each field corresponds to an HTTP method and holds the definition for that operation on this path.
    *   `summary`: `Option<String>` - An optional short summary for all operations on this path.
    *   `description`: `Option<String>` - An optional detailed description for all operations on this path.
    *   `servers`: `Option<Vec<utoipa::openapi::Server>>` - Alternative server list for operations on this path.
    *   `parameters`: `Option<Vec<utoipa::openapi::RefOr<utoipa::openapi::Parameter>>>` - Parameters applicable to all operations on this path.

*   **`utoipa::openapi::Operation`**: Defines a single API operation on a path.
    *   `tags`: `Option<Vec<String>>` - A list of tags for API documentation control.
    *   `summary`: `Option<String>` - A short summary of what the operation does.
    *   `description`: `Option<String>` - A verbose explanation of the operation behavior.
    *   `external_docs`: `Option<utoipa::openapi::ExternalDocs>` - Additional external documentation.
    *   `operation_id`: `Option<String>` - Unique string used to identify the operation.
    *   `parameters`: `Option<Vec<utoipa::openapi::RefOr<utoipa::openapi::Parameter>>>` - Parameters unique to this operation.
    *   `request_body`: `Option<utoipa::openapi::RefOr<utoipa::openapi::request_body::RequestBody>>` - The request body applicable for this operation.
    *   `responses`: `utoipa::openapi::Responses` - **Required.** The list of possible responses for this operation.
    *   `callbacks`: `Option<indexmap::IndexMap<String, utoipa::openapi::RefOr<utoipa::openapi::Callback>>>` - Callbacks that may be initiated by this operation.
    *   `deprecated`: `Option<bool>` - Declares this operation to be deprecated.
    *   `security`: `Option<Vec<utoipa::openapi::security::SecurityRequirement>>` - Security mechanisms for this operation.
    *   `servers`: `Option<Vec<utoipa::openapi::Server>>` - Alternative server list for this operation.

*   **`utoipa::openapi::request_body::RequestBody`**: Describes a single request body.
    *   `description`: `Option<String>` - A brief description of the request body.
    *   `content`: `indexmap::IndexMap<String, utoipa::openapi::path::MediaType>` - **Required.** Describes the content of the request body. Keys are media types (e.g., `application/json`).
    *   `required`: `Option<bool>` - Determines if the request body is required. Defaults to `false`.

*   **`utoipa::openapi::path::MediaType`**: Defines the schema for a specific media type within `RequestBody` or `Response`.
    *   `schema`: `Option<utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>>` - The schema defining the data structure for this media type. This is where you link to a schema in `components`.
    *   `example`: `Option<serde_json::Value>` - Example of the media type's content.
    *   `examples`: `Option<indexmap::IndexMap<String, utoipa::openapi::RefOr<utoipa::openapi::Example>>>` - Multiple examples.
    *   `encoding`: `Option<indexmap::IndexMap<String, utoipa::openapi::Encoding>>` - Encoding information.

*   **`utoipa::openapi::Responses`**: A container for the expected responses of an operation.
    *   `responses`: `indexmap::IndexMap<String, utoipa::openapi::RefOr<utoipa::openapi::Response>>` - Maps HTTP status codes (as strings, e.g., `"200"`, `"500"`, or `"default"`) to `Response` objects (or references).
    *   `default`: `Option<utoipa::openapi::RefOr<utoipa::openapi::Response>>` - A default response definition.

*   **`utoipa::openapi::Response`**: Describes a single response from an API operation.
    *   `description`: `String` - **Required.** A short description of the response.
    *   `headers`: `Option<indexmap::IndexMap<String, utoipa::openapi::RefOr<utoipa::openapi::Header>>>` - Headers that can be sent with the response.
    *   `content`: `Option<indexmap::IndexMap<String, utoipa::openapi::path::MediaType>>` - Describes the content of the response body. Keys are media types.
    *   `links`: `Option<indexmap::IndexMap<String, utoipa::openapi::RefOr<utoipa::openapi::Link>>>` - Links to other operations.

*   **`utoipa::openapi::schema::Schema`**: Defines the structure of data (request/response bodies, parameters). Can be complex (objects, arrays) or simple (primitives). Often constructed via `ToSchema` derive macro.
    *   `schema_type`: `utoipa::openapi::schema::SchemaType` - The fundamental type (Object, Array, String, Number, etc.).
    *   Other fields depend on the `schema_type` (e.g., `properties`, `items`, `format`, `required`).

*   **`utoipa::openapi::schema::SchemaType`**: An enum representing the basic OpenAPI data types (`Object`, `String`, `Integer`, `Number`, `Boolean`, `Array`).

*   **`utoipa::openapi::RefOr<T>`**: An enum that represents either a direct value (`T`) or a reference (`Ref`) to a value defined elsewhere (typically in `Components`).
    *   `Ref(utoipa::openapi::Ref)`: Contains a `$ref` string pointing to a component.
    *   `T(T)`: Contains the actual inline object (`Schema`, `Response`, `RequestBody`, etc.).

*   **`utoipa::openapi::Ref`**: Represents a JSON Reference `$ref`.
    *   `ref_location`: `String` - The reference string (e.g., `#/components/schemas/MySchema`). You typically create this using `Ref::from_schema_name("MySchema")` or similar helpers for other component types.

*   **`utoipa::openapi::Components`**: Holds reusable definitions.
    *   `schemas`: `Option<indexmap::IndexMap<String, utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>>>` - A map where keys are schema names (usually the struct name) and values are the `Schema` definitions (often as `RefOr::T(Schema)`). This is where schemas from `ToSchema` are stored.
    *   `responses`: `Option<indexmap::IndexMap<String, utoipa::openapi::RefOr<utoipa::openapi::Response>>>` - Reusable response definitions.
    *   `parameters`: `Option<indexmap::IndexMap<String, utoipa::openapi::RefOr<utoipa::openapi::Parameter>>>` - Reusable parameter definitions.
    *   And others (`examples`, `request_bodies`, `headers`, `security_schemes`, `links`, `callbacks`).

### 2. Construction and Modification

Here’s the process for manually building the `OpenApi` object:

1.  **Create Initial `OpenApi`:** Instantiate `OpenApi` and set basic info. You can use `OpenApi::new` or construct it manually.

    ```rust
    use utoipa::openapi::{Info, OpenApi, Paths, Components, Server};

    let mut openapi = OpenApi {
        openapi: "3.1.0".to_string(),
        info: Info::new("My Manual API", "1.0.0")
            .description(Some("API built programmatically")),
        servers: Some(vec![
            Server::new("/api/v1")
        ]),
        // Initialize paths and components
        paths: Paths::new(),
        components: Some(Components::new()),
        ..Default::default() // Use default for other optional fields
    };
    ```

2.  **Access Mutable `paths` and `components`:** Get mutable references to the `paths` and `components` fields. You'll need `components` to add schemas derived from `ToSchema`.

    ```rust
    let paths = &mut openapi.paths;
    let components = openapi.components.as_mut().expect("Components are initialized");
    ```

3.  **Add Schemas to `components`:** If you have structs derived with `#[derive(ToSchema)]`, you need to manually get their schema definition and add it to `components.schemas`. The `#[derive(OpenApi)]` macro normally does this automatically by collecting types listed in `#[openapi(components(schemas(...)))]`.

    ```rust
    // Assuming BuildRequest and BuildResponse derive ToSchema
    // components.schemas.insert(
    //     BuildRequest::schema_name().into_owned(), // Get schema name
    //     BuildRequest::schema().1 // Get RefOr<Schema>
    // );
    // components.schemas.insert(
    //     BuildResponse::schema_name().into_owned(),
    //     BuildResponse::schema().1
    // );
    // components.schemas.insert(
    //     ErrorResponse::schema_name().into_owned(),
    //     ErrorResponse::schema().1
    // );
    ```
    *Note: `schema()` returns a tuple `(String, RefOr<Schema>)`. We need the second element.*

4.  **Construct `PathItem`:** Create a `PathItem` for the desired URL path.

    ```rust
    use utoipa::openapi::PathItem;

    let mut path_item = PathItem::new();
    ```

5.  **Construct `Operation`:** Create an `Operation` for the specific HTTP method (e.g., POST). Define its `summary`, `description`, `operation_id`, `tags`, etc.

    ```rust
    use utoipa::openapi::{Operation, Ref, RefOr};
    use utoipa::openapi::request_body::RequestBody;
    use utoipa::openapi::path::MediaType;
    use utoipa::openapi::{Response, Responses};
    use indexmap::indexmap; // For convenient map creation

    let operation = Operation {
        summary: Some("Submit a build job".to_string()),
        description: Some("Creates a new build job based on the provided configuration.".to_string()),
        operation_id: Some("createBuild".to_string()),
        tags: Some(vec!["Builds".to_string()]),
        // ... define requestBody and responses next ...
        ..Default::default()
    };
    ```

6.  **Define `requestBody`:** Create a `RequestBody` specifying the content type and linking to the schema in `components`. Use `RefOr::Ref` and `Ref::from_schema_name`.

    ```rust
    let request_body = RequestBody::new()
        .description(Some("Build configuration"))
        .required(Some(true))
        .content("application/json", MediaType::new()
            .schema(Some(RefOr::Ref(
                Ref::from_schema_name("BuildRequest") // Reference the schema added earlier
            )))
        );

    // Add to the operation (wrapping in RefOr::T for inline definition)
    let mut operation = operation; // Make mutable
    operation.request_body = Some(RefOr::T(request_body));
    ```

7.  **Define `responses`:** Create a `Responses` object. For each status code, create a `Response`, set its `description`, and link its `content` to the appropriate schema in `components`.

    ```rust
    let responses = Responses {
        responses: indexmap! {
            // Success response (200 OK)
            "200".to_string() => RefOr::T(
                Response::new("Build job created successfully")
                    .content("application/json", MediaType::new()
                        .schema(Some(RefOr::Ref(
                            Ref::from_schema_name("BuildResponse") // Reference success schema
                        )))
                    )
            ),
            // Error response (e.g., 500)
            "500".to_string() => RefOr::T(
                Response::new("Internal server error")
                    .content("application/json", MediaType::new()
                        .schema(Some(RefOr::Ref(
                            Ref::from_schema_name("ErrorResponse") // Reference error schema
                        )))
                    )
            )
            // Add more status codes as needed
        },
        ..Default::default()
    };

    // Add to the operation
    operation.responses = responses;
    ```

8.  **Add `Operation` to `PathItem`:** Assign the constructed `Operation` to the appropriate HTTP method field in the `PathItem`.

    ```rust
    path_item.post = Some(operation); // Add the POST operation
    ```

9.  **Add `PathItem` to `Paths`:** Insert the `PathItem` into the `paths.paths` map using the URL path string as the key.

    ```rust
    paths.paths.insert("/v1/build".to_string(), path_item);
    ```

10. **Finalize:** The `openapi` object now contains the manually added path definition. You can serialize it to JSON or YAML.

    ```rust
    // Example: Serialize to pretty JSON
    // let json_string = openapi.to_pretty_json().unwrap();
    // println!("{}", json_string);
    ```

### 3. Examples

Here's a concise example putting it all together:

```rust
use utoipa::openapi::{
    self, // Import common types like Info, Server, PathItem, Operation, etc.
    schema::{ObjectBuilder, Schema, SchemaType}, // For manual schema building if needed
    RefOr, Ref, Components, OpenApi, Paths, Response, Responses
};
use utoipa::openapi::request_body::RequestBody;
use utoipa::openapi::path::MediaType;
use utoipa::ToSchema; // To derive schemas
use indexmap::indexmap; // For map literals

// --- Schemas (Usually defined elsewhere and derived) ---
#[derive(ToSchema)]
struct BuildRequest {
    #[schema(example = "my-project")]
    project_name: String,
    #[schema(example = 10)]
    priority: i32,
}

#[derive(ToSchema)]
struct BuildResponse {
    #[schema(example = "job-12345")]
    job_id: String,
    status: String,
}

#[derive(ToSchema)]
struct ErrorResponse {
    code: i32,
    message: String,
}

// --- Main OpenAPI Construction Logic ---
fn build_manual_openapi() -> OpenApi {
    let mut openapi = OpenApi {
        openapi: "3.1.0".to_string(),
        info: openapi::Info::new("My Manual Build API", "0.1.0")
            .description(Some("API for submitting build jobs, built programmatically.")),
        servers: Some(vec![openapi::Server::new("/api")]),
        paths: Paths::new(),
        components: Some(Components::new()),
        tags: Some(vec![
            openapi::Tag::new("Builds").description(Some("Operations related to build jobs"))
        ]),
        ..Default::default()
    };

    // 1. Get mutable references
    let paths = &mut openapi.paths;
    let components = openapi.components.as_mut().unwrap();

    // 2. Manually add derived schemas to components
    //    (Normally done by #[derive(OpenApi)])
    components.schemas.insert(
        BuildRequest::schema_name().into_owned(),
        BuildRequest::schema().1, // Get the RefOr<Schema> part
    );
    components.schemas.insert(
        BuildResponse::schema_name().into_owned(),
        BuildResponse::schema().1,
    );
    components.schemas.insert(
        ErrorResponse::schema_name().into_owned(),
        ErrorResponse::schema().1,
    );

    // 3. Create Request Body referencing the schema
    let request_body = RequestBody::new()
        .description(Some("Build job configuration"))
        .required(Some(true))
        .content("application/json", MediaType::new()
            .schema(Some(RefOr::Ref(
                Ref::from_schema_name("BuildRequest") // Reference by name
            )))
        );

    // 4. Create Responses referencing schemas
    let responses = Responses {
        responses: indexmap! {
            "201".to_string() => RefOr::T( // 201 Created might be more appropriate
                Response::new("Build job accepted")
                    .content("application/json", MediaType::new()
                        .schema(Some(RefOr::Ref(
                            Ref::from_schema_name("BuildResponse")
                        )))
                    )
            ),
            "400".to_string() => RefOr::T( // Example error
                Response::new("Bad Request - Invalid input")
                    .content("application/json", MediaType::new()
                        .schema(Some(RefOr::Ref(
                            Ref::from_schema_name("ErrorResponse")
                        )))
                    )
            ),
             "500".to_string() => RefOr::T( // Example error
                Response::new("Internal Server Error")
                    .content("application/json", MediaType::new()
                        .schema(Some(RefOr::Ref(
                            Ref::from_schema_name("ErrorResponse")
                        )))
                    )
            ),
        },
        ..Default::default()
    };

    // 5. Create the Operation
    let operation = Operation {
        summary: Some("Submit a new build job".to_string()),
        operation_id: Some("submitBuildJob".to_string()),
        tags: Some(vec!["Builds".to_string()]),
        request_body: Some(RefOr::T(request_body)), // Use the constructed body
        responses, // Use the constructed responses
        ..Default::default()
    };

    // 6. Create PathItem and add the Operation
    let mut path_item = PathItem::new();
    path_item.post = Some(operation);

    // 7. Add PathItem to Paths
    paths.paths.insert("/v1/build".to_string(), path_item);

    // Add more paths and operations as needed...

    openapi // Return the fully constructed object
}

fn main() {
    let api_doc = build_manual_openapi();
    match api_doc.to_pretty_json() {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Failed to serialize OpenAPI spec: {}", e),
    }
}

```

This example demonstrates the core flow: initializing `OpenApi`, manually adding `ToSchema`-derived schemas to `components`, building `RequestBody` and `Responses` using references (`RefOr::Ref`) to those schemas, assembling them into an `Operation`, placing the `Operation` into a `PathItem`, and finally inserting the `PathItem` into the main `paths` map.
