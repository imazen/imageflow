//! Issue #626: BMP (DIB) files must decode in the default build. User-submitted
//! content is sometimes a bitmap wearing a `.jpg` extension; detection is by
//! magic bytes, so the extension never matters — only the enabled decoder set.

use imageflow_core::Context;
use imageflow_types as s;

/// Hand-assemble a 24-bit, bottom-up, BI_RGB BMP so the test needs no fixtures.
/// `rows` is top-to-bottom, each pixel `(r, g, b)`.
fn build_bmp24(rows: &[Vec<(u8, u8, u8)>]) -> Vec<u8> {
    let h = rows.len() as u32;
    let w = rows[0].len() as u32;
    let row_bytes = (w * 3).div_ceil(4) * 4; // rows are padded to 4 bytes
    let pixel_bytes = row_bytes * h;
    let file_size = 14 + 40 + pixel_bytes;

    let mut out = Vec::with_capacity(file_size as usize);
    // BITMAPFILEHEADER
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes()); // pixel data offset

    // BITMAPINFOHEADER
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes()); // positive = bottom-up
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&24u16.to_le_bytes()); // bpp
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&pixel_bytes.to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes()); // 72 dpi
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // colors used
    out.extend_from_slice(&0u32.to_le_bytes()); // important colors

    // Pixel rows, bottom-up, BGR order.
    for row in rows.iter().rev() {
        let start = out.len();
        for &(r, g, b) in row {
            out.extend_from_slice(&[b, g, r]);
        }
        while (out.len() - start) < row_bytes as usize {
            out.push(0);
        }
    }
    assert_eq!(out.len(), file_size as usize);
    out
}

fn decode_to_png(bmp: Vec<u8>) -> Vec<u8> {
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, bmp).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(s::Execute001 {
        graph_recording: None,
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode { io_id: 1, preset: s::EncoderPreset::libpng32() },
        ]),
    })
    .unwrap();
    ctx.take_output_buffer(1).unwrap()
}

#[test]
fn bmp_decodes_in_default_build_with_correct_pixels() {
    // 3x2, one distinct color per pixel, so row order and channel order are both checked.
    let rows = vec![
        vec![(255, 0, 0), (0, 255, 0), (0, 0, 255)],
        vec![(255, 255, 0), (0, 255, 255), (32, 64, 96)],
    ];
    let png = decode_to_png(build_bmp24(&rows));

    let decoded = lodepng::decode32(&png).unwrap();
    assert_eq!((decoded.width, decoded.height), (3, 2));
    for (y, row) in rows.iter().enumerate() {
        for (x, &(r, g, b)) in row.iter().enumerate() {
            let p = decoded.buffer[y * 3 + x];
            assert_eq!(
                (p.r, p.g, p.b, p.a),
                (r, g, b, 255),
                "pixel ({x},{y}) mismatch: BMP row order or BGR→RGB swizzle is wrong"
            );
        }
    }
}

#[test]
fn bmp_get_image_info_reports_dimensions_and_mime() {
    let rows = vec![vec![(10, 20, 30); 5]; 4];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, build_bmp24(&rows)).unwrap();
    let info = ctx.get_unscaled_rotated_image_info(0).unwrap();
    assert_eq!((info.image_width, info.image_height), (5, 4));
    assert_eq!(info.preferred_mime_type, "image/bmp");
    assert_eq!(info.preferred_extension, "bmp");
}

#[test]
fn bmp_with_jpeg_extension_resizes_via_querystring() {
    // The reporter's case: a `.jpg` that is really a DIB. Detection is by magic
    // bytes, so run it through the RIAPI path with a resize and a JPEG output.
    let rows = vec![vec![(200, 100, 50); 16]; 12];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, build_bmp24(&rows)).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(s::Execute001 {
        graph_recording: None,
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(vec![s::Node::CommandString {
            kind: s::CommandStringKind::ImageResizer4,
            value: "w=8&h=6&format=jpg".to_owned(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None,
        }]),
    })
    .unwrap();
    let out = ctx.take_output_buffer(1).unwrap();
    assert!(out.starts_with(b"\xFF\xD8\xFF"), "expected JPEG output");
}
