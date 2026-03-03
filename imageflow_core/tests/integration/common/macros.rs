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
/// The identity is used by `compare_encoded_v2` and friends to build
/// structured `.checksums` keys.
macro_rules! test_identity {
    () => {{
        // Get function name via type_name_of_val on a local fn
        fn __f() {}
        let full = std::any::type_name_of_val(&__f);
        // full = "integration::visuals::trim::test_trim_whitespace::__f"
        let without_f = &full[..full.len() - 4]; // strip "::__f"
        let func_name = match without_f.rsplit("::").next() {
            Some(n) => n,
            None => without_f,
        };

        // Module stem from file!()
        let file_path = file!();
        let stem = match file_path.rfind('/') {
            Some(pos) => &file_path[pos + 1..],
            None => file_path,
        };
        let stem = match stem.strip_suffix(".rs") {
            Some(s) => s,
            None => stem,
        };

        $crate::common::TestIdentity {
            module: stem,
            func_name,
        }
    }};
}

/// Run a visual check test with automatic identity derivation.
///
/// Thin wrapper around `compare_encoded_v2` that derives module/function
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
/// - `similarity`: `Similarity` value (default: `AllowDssimMatch(0.0, 0.002)`)
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

        $crate::common::compare_encoded_v2(
            Some($crate::common::IoTestEnum::Url(source_url)),
            &identity,
            detail,
            $crate::common::Constraints {
                similarity,
                max_file_size,
            },
            steps,
        );
    }};

    // Default detail: empty string
    (@detail) => { "" };
    (@detail $detail:expr) => { $detail };

    // Default similarity: AllowDssimMatch(0.0, 0.002)
    (@similarity) => { $crate::common::Similarity::AllowDssimMatch(0.0, 0.002) };
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

        $crate::common::compare_encoded_v2(
            Some($crate::common::IoTestEnum::Url(source_url)),
            &identity,
            detail,
            $crate::common::Constraints {
                similarity,
                max_file_size,
            },
            $steps,
        );
    }};
}

/// Run a visual check on a bitmap result (not encoded output).
///
/// Uses `evaluate_result_v2` with bitmap comparison instead of
/// encoded byte comparison.
macro_rules! visual_check_bitmap {
    (
        $( source: $source:expr, )?
        $( detail: $detail:expr, )?
        steps: $steps:expr,
        $( allowed_off_by_one: $obo:expr, )?
    ) => {{
        let identity = test_identity!();
        let detail: &str = visual_check!(@detail $( $detail )?);
        let allowed: usize = visual_check_bitmap!(@obo $( $obo )?);

        let inputs: Vec<$crate::common::IoTestEnum> = vec![
            $( $crate::common::IoTestEnum::Url(visual_check!(@source_url $source)), )?
        ];

        let mut context = imageflow_core::Context::create().unwrap();
        let mut bit = $crate::common::BitmapBgraContainer::empty();
        let mut steps_vec: Vec<imageflow_types::Node> = $steps;
        steps_vec.push(unsafe { bit.as_mut().get_node() });

        let response = $crate::common::build_steps(
            &mut context, &steps_vec, inputs, None, false,
        ).unwrap();

        let bitmap_key = bit.bitmap_key(&context)
            .unwrap_or_else(|| panic!("execution failed {:?}", response));

        let ctx = $crate::common::ChecksumCtx::visuals();
        $crate::common::evaluate_result_v2(
            &ctx,
            identity.module,
            identity.func_name,
            detail,
            $crate::common::ResultKind::Bitmap { context: &context, key: bitmap_key },
            $crate::common::Constraints {
                similarity: $crate::common::Similarity::AllowOffByOneBytesCount(allowed as i64),
                max_file_size: None,
            },
            true,
        );
    }};

    (@obo) => { 500usize };
    (@obo $obo:expr) => { $obo };
}
