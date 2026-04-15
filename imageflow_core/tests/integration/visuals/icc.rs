//! ICC profile and wide-gamut color management tests.
//!
//! Exercises imageflow's ICC profile detection and color conversion pipeline
//! using real images with embedded ICC profiles (Display P3, Adobe RGB,
//! ProPhoto, Rec.2020, sRGB reference cameras).
//!
//! Images are from s3://imageflow-resources/test_inputs/wide-gamut/ and
//! s3://imageflow-resources/test_inputs/repro-icc/.
//!
//! These tests verify:
//! - ICC-tagged images decode without crash
//! - Color management produces stable cross-platform results (via checksums)
//! - Different ICC profiles produce different decode results
//! - Re-encoding ICC-decoded images is deterministic

#[allow(unused_imports)]
use crate::common::*;
use imageflow_types::{EncoderPreset, Node};

// ─── Display P3 images ───────────────────────────────────────────────
// These JPEGs have embedded Display P3 ICC profiles. Imageflow should
// detect the profile and convert to sRGB during decode.

#[test]
fn test_icc_display_p3_decode_1() {
    visual_check! {
        source: "test_inputs/wide-gamut/display-p3/flickr_1b94e1228c32cb98.jpg",
        detail: "p3_decode",
        command: "format=png",
        similarity: Similarity::MaxZdsim(0.02),
    }
}

#[test]
fn test_icc_display_p3_decode_2() {
    visual_check! {
        source: "test_inputs/wide-gamut/display-p3/flickr_2fc1b8c45f922b8e.jpg",
        detail: "p3_decode",
        command: "format=png",
        similarity: Similarity::MaxZdsim(0.03),
    }
}

#[test]
fn test_icc_display_p3_decode_3() {
    visual_check! {
        source: "test_inputs/wide-gamut/display-p3/flickr_3ac029fc145a8e32.jpg",
        detail: "p3_decode",
        command: "format=png",
        similarity: Similarity::MaxZdsim(0.03),
    }
}

#[test]
fn test_icc_display_p3_resize() {
    visual_check! {
        source: "test_inputs/wide-gamut/display-p3/flickr_403aa5efb8efe6e8.jpg",
        detail: "p3_resize_400",
        command: "w=400&format=png",
        similarity: Similarity::MaxZdsim(0.02),
    }
}

#[test]
fn test_icc_display_p3_resize_filter() {
    visual_check! {
        source: "test_inputs/wide-gamut/display-p3/flickr_47b2cd2c048f29b3.jpg",
        detail: "p3_robidoux_300x300",
        command: "w=300&h=300&mode=crop&filter=Robidoux&format=png",
        similarity: Similarity::MaxZdsim(0.02),
    }
}

// ─── Adobe RGB images ────────────────────────────────────────────────
// Adobe RGB has a different gamut than sRGB. Imageflow should convert
// to sRGB via the embedded ICC profile.

#[test]
fn test_icc_adobe_rgb_decode_1() {
    visual_check! {
        source: "test_inputs/wide-gamut/adobe-rgb/flickr_0119a8378404ece9.jpg",
        detail: "adobergb_decode",
        command: "format=png",
        similarity: Similarity::MaxZdsim(0.02),
    }
}

#[test]
fn test_icc_adobe_rgb_decode_2() {
    visual_check! {
        source: "test_inputs/wide-gamut/adobe-rgb/flickr_070040b3922aab8a.jpg",
        detail: "adobergb_decode",
        command: "format=png",
        similarity: Similarity::MaxZdsim(0.02),
    }
}

#[test]
fn test_icc_adobe_rgb_resize() {
    visual_check! {
        source: "test_inputs/wide-gamut/adobe-rgb/flickr_083f5c58e82b1640.jpg",
        detail: "adobergb_resize_400",
        command: "w=400&format=png",
        similarity: Similarity::MaxZdsim(0.02),
    }
}

// ─── Rec.2020 images ─────────────────────────────────────────────────
// BT.2020/Rec.2020 has the widest gamut of the test set.

#[test]
fn test_icc_rec2020_decode_1() {
    visual_check! {
        source: "test_inputs/wide-gamut/rec-2020-pq/flickr_2a68670c58131566.jpg",
        detail: "rec2020_decode",
        command: "format=png",
        similarity: Similarity::MaxZdsim(0.03),
    }
}

#[test]
fn test_icc_rec2020_decode_2() {
    visual_check! {
        source: "test_inputs/wide-gamut/rec-2020-pq/flickr_c2d8824d6ffb6e60.jpg",
        detail: "rec2020_decode",
        command: "format=png",
        similarity: Similarity::MaxZdsim(0.04),
    }
}

// ─── ProPhoto RGB images ─────────────────────────────────────────────

#[test]
fn test_icc_prophoto_decode() {
    visual_check! {
        source: "test_inputs/wide-gamut/prophoto-rgb/flickr_0d2d634cf46df137.jpg",
        detail: "prophoto_decode",
        command: "format=png",
        similarity: Similarity::MaxZdsim(0.03),
    }
}

#[test]
fn test_icc_prophoto_resize() {
    visual_check! {
        source: "test_inputs/wide-gamut/prophoto-rgb/flickr_6c6ab0d50486564a.jpg",
        detail: "prophoto_resize_400",
        command: "w=400&format=png",
        similarity: Similarity::MaxZdsim(0.02),
    }
}

// ─── sRGB reference images (camera models) ───────────────────────────
// These have explicit sRGB ICC profiles. Should be a no-op conversion,
// useful as a control group.

#[test]
fn test_icc_srgb_canon_5d() {
    visual_check! {
        source: "test_inputs/wide-gamut/srgb-reference/canon_eos_5d_mark_iv/wmc_81b268fc64ea796c.jpg",
        detail: "srgb_canon5d",
        command: "format=png",
        similarity: Similarity::MaxZdsim(0.02),
    }
}

#[test]
fn test_icc_srgb_sony_a7rv() {
    visual_check! {
        source: "test_inputs/wide-gamut/srgb-reference/sony-a7r-v/irsample_a141d146726a8314.jpg",
        detail: "srgb_sony",
        command: "format=png",
        similarity: Similarity::MaxZdsim(0.02),
    }
}

// ─── Repro-icc images from real bug reports ──────────────────────────
// Images that caused real issues in image processing libraries.
// Primarily validates no-crash and stable output.

#[test]
fn test_icc_repro_sharp_icc() {
    visual_check! {
        source: "test_inputs/repro-icc/sharp/1323_115925293-3319d700-a481-11eb-8083-66b5188ee1da.png",
        detail: "sharp_icc",
        command: "format=png",
    }
}

#[test]
fn test_icc_repro_pillow_icc() {
    visual_check! {
        source: "test_inputs/repro-icc/python-pillow/1529_9fa6c9ca-8603-11e5-97e7-589cf9e3baaa.jpg",
        detail: "pillow_icc",
        command: "format=png",
        // ICC transform rounding differs across SIMD paths: sim 95.3 on win-arm64
        similarity: Similarity::MaxZdsim(0.05),
    }
}

#[test]
fn test_icc_repro_imagemagick_icc() {
    visual_check! {
        source: "test_inputs/repro-icc/imagemagick/2161_84902501-90046e00-b0b5-11ea-91c6-c220fd29fd44.jpg",
        detail: "imagemagick_icc",
        command: "format=png",
        similarity: Similarity::MaxZdsim(0.02),
    }
}

#[test]
fn test_icc_repro_libvips_icc() {
    visual_check! {
        source: "test_inputs/repro-icc/libvips/1063_44146319-5742eab6-a08f-11e8-911a-2aaef2a42540.jpg",
        detail: "libvips_icc",
        command: "format=png",
        // ICC transform rounding differs across SIMD paths: sim 97.1 on win-arm64
        similarity: Similarity::MaxZdsim(0.03),
    }
}

// ─── ICC + resize combinations ───────────────────────────────────────
// Tests that ICC conversion interacts correctly with the resize pipeline.

#[test]
fn test_icc_p3_crop_and_resize() {
    visual_check! {
        source: "test_inputs/wide-gamut/display-p3/flickr_769c664daf96b5d5.jpg",
        detail: "p3_crop_500x500",
        command: "w=500&h=500&mode=crop&format=png",
    }
}

#[test]
fn test_icc_adobe_rgb_constrain() {
    visual_check_steps! {
        source: "test_inputs/wide-gamut/adobe-rgb/flickr_092650e9e8211233.jpg",
        detail: "adobergb_constrain_300",
        steps: vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Constrain(imageflow_types::Constraint {
                mode: imageflow_types::ConstraintMode::Within,
                w: Some(300),
                h: Some(300),
                hints: None,
                gravity: None,
                canvas_color: None,
            }),
            Node::Encode { io_id: 1, preset: EncoderPreset::libpng32() },
        ],
        similarity: Similarity::MaxZdsim(0.02),
    }
}

#[test]
fn test_icc_p3_to_jpeg_roundtrip() {
    // Decode P3 JPEG → sRGB via ICC → re-encode as JPEG
    // Tests that color management + lossy re-encode produces stable output
    visual_check! {
        source: "test_inputs/wide-gamut/display-p3/flickr_952bd5d8c41d3e6d.jpg",
        detail: "p3_to_jpeg_q85",
        command: "format=jpg&quality=85",
        similarity: Similarity::MaxZdsim(0.05),
    }
}

#[test]
fn test_icc_p3_to_webp() {
    visual_check! {
        source: "test_inputs/wide-gamut/display-p3/flickr_c585e5e91ff47e1c.jpg",
        detail: "p3_to_webp_q80",
        command: "format=webp&quality=80",
        similarity: Similarity::MaxZdsim(0.05),
    }
}

// ─── Gray + gamma images ─────────────────────────────────────────────

#[test]
fn test_icc_gray_gamma22_decode() {
    visual_check! {
        source: "test_inputs/wide-gamut/gray-gamma-22/flickr_2f4bbf638f18ebea.jpg",
        detail: "gray_gamma22",
        command: "format=png",
        similarity: Similarity::MaxZdsim(0.02),
    }
}
