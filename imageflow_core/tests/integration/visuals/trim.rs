#[allow(unused_imports)]
use crate::common::*;

#[test]
fn test_trim_whitespace() {
    visual_check! {
        source: "test_inputs/shirt_transparent.png",
        detail: "transparent_shirt",
        command: "trim.threshold=80",
    }
}

#[test]
fn test_trim_whitespace_with_padding() {
    visual_check! {
        source: "test_inputs/whitespace-issue.png",
        detail: "gray_bg",
        command: "trim.threshold=20&trim.percentpadding=0.5&bgcolor=gray",
    }
}

#[test]
fn test_trim_resize_whitespace_with_padding() {
    visual_check! {
        source: "test_inputs/whitespace-issue.png",
        detail: "450x450_gray",
        command: "w=450&h=450&scale=both&trim.threshold=20&trim.percentpadding=10&bgcolor=gray",
    }
}

#[test]
fn test_trim_resize_whitespace_without_padding() {
    visual_check! {
        source: "test_inputs/whitespace-issue.png",
        detail: "450x450_gray",
        command: "w=450&h=450&scale=both&trim.threshold=20&bgcolor=gray",
    }
}

#[test]
fn test_trim_whitespace_with_padding_no_resize() {
    visual_check! {
        source: "test_inputs/whitespace-issue.png",
        detail: "gray_bg",
        command: "trim.threshold=20&trim.percentpadding=0.5&bgcolor=gray",
    }
}
