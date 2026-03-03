use crate::common::*;
use imageflow_types::{CommandStringKind, Node};

const DEBUG_GRAPH: bool = false;
const POPULATE_CHECKSUMS: bool = true;

#[test]
fn test_trim_whitespace() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/shirt_transparent.png".to_owned())),
        "test_trim_whitespace transparent_shirt",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowDssimMatch(0.0, 0.002),
            max_file_size: None
        },
        vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "trim.threshold=80".to_owned(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None
            }
        ]
    );
}

#[test]
fn test_trim_whitespace_with_padding() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/whitespace-issue.png".to_owned())),
        "test_trim_whitespace_with_padding gray_bg",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowDssimMatch(0.0, 0.002),
            max_file_size: None
        },
        vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "trim.threshold=20&trim.percentpadding=0.5&bgcolor=gray".to_owned(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None
            }
        ]
    );
}

#[test]
fn test_trim_resize_whitespace_with_padding() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/whitespace-issue.png".to_owned())),
        "test_trim_resize_whitespace_with_padding 450x450_gray",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowDssimMatch(0.0, 0.002),
            max_file_size: None
        },
        vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "w=450&h=450&scale=both&trim.threshold=20&trim.percentpadding=10&bgcolor=gray".to_owned(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None
            }
        ]
    );
}

#[test]
fn test_trim_resize_whitespace_without_padding() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/whitespace-issue.png".to_owned())),
        "test_trim_resize_whitespace_without_padding 450x450_gray",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowDssimMatch(0.0, 0.002),
            max_file_size: None
        },
        vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "w=450&h=450&scale=both&trim.threshold=20&bgcolor=gray".to_owned(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None
            }
        ]
    );
}

#[test]
fn test_trim_whitespace_with_padding_no_resize() {
    compare_encoded(
        Some(IoTestEnum::Url("https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/whitespace-issue.png".to_owned())),
        "test_trim_whitespace_with_padding_no_resize gray_bg",
        POPULATE_CHECKSUMS,
        DEBUG_GRAPH,
        Constraints {
            similarity: Similarity::AllowDssimMatch(0.0, 0.002),
            max_file_size: None
        },
        vec![
            Node::CommandString{
                kind: CommandStringKind::ImageResizer4,
                value: "trim.threshold=20&trim.percentpadding=0.5&bgcolor=gray".to_owned(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None
            }
        ]
    );
}
