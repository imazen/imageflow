//! Visual test macros that eliminate test name boilerplate.
//!
//! These macros derive the module name and function name at compile time,
//! then forward to `#[track_caller]` functions for proper panic locations.
//!
//! # Usage
//!
//! ```ignore
//! #[test]
//! fn test_trim_whitespace() {
//!     visual_check! {
//!         source: "test_inputs/shirt_transparent.png",
//!         detail: "transparent_shirt",
//!         command: "trim.threshold=80",
//!     }
//! }
//! ```

/// Derives (module_stem, function_name) from the call site at compile time.
///
/// Returns a `TestIdentity` with the file stem (e.g., `"trim"` from
/// `visuals/trim.rs`) and function name extracted via `type_name_of_val`.
///
/// The identity is used by `compare_encoded` and friends to build
/// structured `.checksums` keys.
macro_rules! test_identity {
    () => {{
        // Get function name via type_name_of_val on a local fn
        fn __f() {}
        let full = std::any::type_name_of_val(&__f);
        // full = "integration::visuals::trim::test_trim_whitespace::__f"
        let without_f = full.strip_suffix("::__f").unwrap_or(full);
        let func_name = match without_f.rsplit("::").next() {
            Some(n) => n,
            None => without_f,
        };

        // Module stem from file!() — handle both '/' (unix) and '\' (windows)
        let file_path = file!();
        let last_sep = match (file_path.rfind('/'), file_path.rfind('\\')) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        let stem = match last_sep {
            Some(pos) => &file_path[pos + 1..],
            None => file_path,
        };
        let stem = match stem.strip_suffix(".rs") {
            Some(s) => s,
            None => stem,
        };

        $crate::common::TestIdentity { module: stem, func_name }
    }};
}

/// Run a visual check test with automatic identity derivation.
///
/// Thin wrapper around `compare_encoded` that derives module/function
/// names at compile time via `test_identity!`.
///
/// # Required fields
///
/// - `source`: Path relative to S3 base URL, or full URL
/// - `command`: ImageResizer4 command string (query params)
///
/// # Optional fields
///
/// - `detail`: Discriminant for multiple comparisons in one test (default: "")
/// - `similarity`: `Similarity` value (default: `MaxZdsim(0.01)`)
/// - `max_file_size`: Maximum encoded file size in bytes
///
/// # Examples
///
/// ```ignore
/// // Basic usage:
/// visual_check! {
///     source: "test_inputs/shirt_transparent.png",
///     command: "trim.threshold=80",
/// }
///
/// // With discriminant:
/// visual_check! {
///     source: "test_inputs/waterhouse.jpg",
///     detail: "robidoux_400x300",
///     command: "w=400&h=300&filter=Robidoux",
/// }
/// ```
macro_rules! visual_check {
    (
        source: $source:expr,
        $( detail: $detail:expr, )?
        command: $command:expr,
        $( similarity: $similarity:expr, )?
        $( max_file_size: $max_file_size:expr, )?
    ) => {{
        let identity = test_identity!();
        let detail: &str = visual_check!(@detail $( $detail )?);
        let source_url = visual_check!(@source_url $source);

        let similarity = visual_check!(@similarity $( $similarity )?);
        let max_file_size: Option<usize> = visual_check!(@max_file_size $( $max_file_size )?);

        let steps = vec![
            imageflow_types::Node::CommandString {
                kind: imageflow_types::CommandStringKind::ImageResizer4,
                value: $command.to_owned(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None,
            }
        ];

        $crate::common::compare_encoded(
            Some($crate::common::IoTestEnum::Url(source_url.clone())),
            &identity,
            detail,
            Some(&source_url),
            $crate::common::Constraints {
                similarity: Some(similarity),
                max_file_size,
            },
            steps,
        );
    }};

    // Default detail: empty string
    (@detail) => { "" };
    (@detail $detail:expr) => { $detail };

    // Default similarity: zdsim <= 0.01 (zensim >= 99). Most tests produce
    // identical output cross-platform. Tests with known lossy differences
    // should set an explicit similarity threshold.
    (@similarity) => { $crate::common::Similarity::MaxZdsim(0.01) };
    (@similarity $similarity:expr) => { $similarity };

    // Default max_file_size: None
    (@max_file_size) => { None };
    (@max_file_size $max_file_size:expr) => { Some($max_file_size) };

    // Source URL resolution: prepend S3 base if not already a full URL
    (@source_url $source:expr) => {{
        let s: &str = $source;
        if s.starts_with("http://") || s.starts_with("https://") {
            s.to_owned()
        } else {
            format!(
                "https://s3-us-west-2.amazonaws.com/imageflow-resources/{}",
                s
            )
        }
    }};
}

/// Run a visual check with custom `Node` steps instead of a command string.
///
/// For tests that need non-CommandString nodes (e.g., Resample2D, FillRect,
/// Watermark, multiple steps).
///
/// # Required fields
///
/// - `source`: Path relative to S3 base URL, or full URL
/// - `steps`: `Vec<Node>` to execute (must include decode and encode)
///
/// # Optional fields
///
/// - `detail`: Discriminant for multiple comparisons in one test
/// - `similarity`: `Similarity` value
/// - `max_file_size`: Maximum encoded file size
macro_rules! visual_check_steps {
    (
        source: $source:expr,
        $( detail: $detail:expr, )?
        steps: $steps:expr,
        $( similarity: $similarity:expr, )?
        $( max_file_size: $max_file_size:expr, )?
    ) => {{
        let identity = test_identity!();
        let detail: &str = visual_check!(@detail $( $detail )?);
        let source_url = visual_check!(@source_url $source);

        let similarity = visual_check!(@similarity $( $similarity )?);
        let max_file_size: Option<usize> = visual_check!(@max_file_size $( $max_file_size )?);

        $crate::common::compare_encoded(
            Some($crate::common::IoTestEnum::Url(source_url.clone())),
            &identity,
            detail,
            Some(&source_url),
            $crate::common::Constraints {
                similarity: Some(similarity),
                max_file_size,
            },
            $steps,
        );
    }};
}

/// Run a visual check on a bitmap result (not encoded output).
///
/// Thin wrapper around `compare_bitmap` that derives module/function
/// names at compile time via `test_identity!`.
///
/// # Variants
///
/// - Single source: `source: "path/to/image.jpg",`
/// - Multiple sources: `sources: ["path1.jpg", "path2.png"],`
/// - No source (canvas tests): omit `source:`/`sources:`
///
/// # Required fields
///
/// - `steps`: `Vec<Node>` to execute
/// - `tolerance`: `Tolerance` value (e.g., `Tolerance::off_by_one()`)
///
/// # Optional fields
///
/// - `detail`: Discriminant for multiple comparisons (default: "")
macro_rules! visual_check_bitmap {
    // Single source variant
    (
        source: $source:expr,
        $( detail: $detail:expr, )?
        steps: $steps:expr,
        tolerance: $tol:expr,
    ) => {{
        let identity = test_identity!();
        let detail: &str = visual_check!(@detail $( $detail )?);
        let tolerance = $tol;
        let source_url = visual_check!(@source_url $source);
        let inputs = vec![
            $crate::common::IoTestEnum::Url(source_url.clone()),
        ];
        $crate::common::compare_bitmap(inputs, &identity, detail, Some(&source_url), $steps, &tolerance);
    }};

    // Multi-source variant (e.g., watermark tests)
    (
        sources: [$( $source:expr ),+ $(,)?],
        $( detail: $detail:expr, )?
        steps: $steps:expr,
        tolerance: $tol:expr,
    ) => {{
        let identity = test_identity!();
        let detail: &str = visual_check!(@detail $( $detail )?);
        let tolerance = $tol;
        let inputs = vec![
            $( $crate::common::IoTestEnum::Url(visual_check!(@source_url $source)), )+
        ];
        // Multi-source: no single source for zensim comparison
        $crate::common::compare_bitmap(inputs, &identity, detail, None, $steps, &tolerance);
    }};

    // No source variant (canvas tests)
    (
        $( detail: $detail:expr, )?
        steps: $steps:expr,
        tolerance: $tol:expr,
    ) => {{
        let identity = test_identity!();
        let detail: &str = visual_check!(@detail $( $detail )?);
        let tolerance = $tol;
        $crate::common::compare_bitmap(vec![], &identity, detail, None, $steps, &tolerance);
    }};
}
