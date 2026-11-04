//! Embedded ICC profiles that cannot describe the image's samples.
//!
//! A CMYK profile on a 3-component or grayscale JPEG (imageflow#737) is ignored: the
//! image decodes exactly as it does without the profile.

use crate::common::decode_image;
use imageflow_core::Context;

static CMYK_ICC: &[u8] = include_bytes!("../../src/codecs/cmyk.icc");

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;

/// Insert `icc` after the SOI marker as APP2 ICC_PROFILE segments.
fn embed_icc_in_jpeg(jpeg: &[u8], icc: &[u8]) -> Vec<u8> {
    const HEADER: &[u8] = b"ICC_PROFILE\0";
    let chunks: Vec<&[u8]> = icc.chunks(0xFFFF - 2 - HEADER.len() - 2).collect();
    let count = u8::try_from(chunks.len()).expect("an ICC profile fits in 255 segments");

    let mut out = jpeg[..2].to_vec();
    for (sequence, chunk) in (1u8..).zip(&chunks) {
        let length = 2 + HEADER.len() + 2 + chunk.len();
        out.extend_from_slice(&[0xFF, 0xE2]);
        out.extend_from_slice(&u16::try_from(length).unwrap().to_be_bytes());
        out.extend_from_slice(HEADER);
        out.extend_from_slice(&[sequence, count]);
        out.extend_from_slice(chunk);
    }
    out.extend_from_slice(&jpeg[2..]);
    out
}

fn encode_jpeg(samples: &[u8], color: image::ExtendedColorType) -> Vec<u8> {
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90)
        .encode(samples, WIDTH, HEIGHT, color)
        .unwrap();
    jpeg
}

fn decoded_pixels(bytes: &[u8]) -> Vec<u8> {
    let mut ctx = Context::create().unwrap();
    ctx.add_copied_input_buffer(0, bytes).unwrap();
    let key = decode_image(&mut ctx, 0);
    let bitmaps = ctx.borrow_bitmaps().unwrap();
    let mut bitmap = bitmaps.try_borrow_mut(key).unwrap();
    let mut window = bitmap.get_window_u8().unwrap();
    window.normalize_unused_alpha().unwrap();
    window.scanlines().flat_map(|line| line.row().to_vec()).collect()
}

fn assert_profile_is_ignored(jpeg: &[u8]) {
    let tagged = embed_icc_in_jpeg(jpeg, CMYK_ICC);
    assert!(
        decoded_pixels(&tagged) == decoded_pixels(jpeg),
        "the profile changed the decoded pixels"
    );
}

#[test]
fn cmyk_profile_on_ycbcr_jpeg_is_ignored() {
    let rgb: Vec<u8> = (0..HEIGHT)
        .flat_map(|y| (0..WIDTH).flat_map(move |x| [(x * 4) as u8, (y * 4) as u8, (x + y) as u8]))
        .collect();
    assert_profile_is_ignored(&encode_jpeg(&rgb, image::ExtendedColorType::Rgb8));
}

#[test]
fn cmyk_profile_on_grayscale_jpeg_is_ignored() {
    let gray: Vec<u8> =
        (0..HEIGHT).flat_map(|y| (0..WIDTH).map(move |x| (x * 2 + y * 2) as u8)).collect();
    assert_profile_is_ignored(&encode_jpeg(&gray, image::ExtendedColorType::L8));
}
