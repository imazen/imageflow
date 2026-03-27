//! Shared color parsing utilities for the zen pipeline.
//!
//! Centralizes hex color parsing used by translate.rs, converter.rs, and execute.rs.

use imageflow_types::{self as s, Color};

/// Parse a hex color string to RGBA bytes.
///
/// Accepts: `#RRGGBB`, `#RRGGBBAA`, or bare `RRGGBB`/`RRGGBBAA`.
/// Returns `[R, G, B, A]` with A=255 when not specified.
pub fn parse_hex_rgba(hex: &str) -> [u8; 4] {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(hex.get(0..2).unwrap_or("00"), 16).unwrap_or(0);
    let g = u8::from_str_radix(hex.get(2..4).unwrap_or("00"), 16).unwrap_or(0);
    let b = u8::from_str_radix(hex.get(4..6).unwrap_or("00"), 16).unwrap_or(0);
    let a = if hex.len() >= 8 { u8::from_str_radix(&hex[6..8], 16).unwrap_or(255) } else { 255 };
    [r, g, b, a]
}

/// Convert a v2 [`Color`] to RGBA bytes.
pub fn color_to_rgba(color: &Color) -> [u8; 4] {
    match color {
        Color::Transparent => [0, 0, 0, 0],
        Color::Black => [0, 0, 0, 255],
        Color::Srgb(s::ColorSrgb::Hex(hex)) => parse_hex_rgba(hex),
    }
}

/// Convert a v2 [`Color`] to a CSS-style string for zenlayout node params.
///
/// Returns `"transparent"`, `"#000000FF"`, or `"#RRGGBB[AA]"`.
pub fn color_to_css_string(color: &Color) -> String {
    match color {
        Color::Transparent => "transparent".to_string(),
        Color::Black => "#000000FF".to_string(),
        Color::Srgb(s::ColorSrgb::Hex(hex)) => {
            let hex = hex.trim_start_matches('#');
            format!("#{hex}")
        }
    }
}

/// Convert a v2 [`Color`] to RGB bytes (no alpha), or None for transparent.
pub fn color_to_rgb(color: &Color) -> Option<[u8; 3]> {
    match color {
        Color::Transparent => None,
        Color::Black => Some([0, 0, 0]),
        Color::Srgb(s::ColorSrgb::Hex(hex)) => {
            let rgba = parse_hex_rgba(hex);
            Some([rgba[0], rgba[1], rgba[2]])
        }
    }
}

/// Parse a CSS-style color string to RGBA bytes.
///
/// Accepts: `"transparent"`, `"white"`, `"black"`, `"#RRGGBB"`, `"#RRGGBBAA"`.
pub fn parse_css_color(s: &str) -> [u8; 4] {
    match s.to_lowercase().as_str() {
        "transparent" | "" => [0, 0, 0, 0],
        "white" => [255, 255, 255, 255],
        "black" => [0, 0, 0, 255],
        hex if hex.starts_with('#') => parse_hex_rgba(hex),
        _ => [0, 0, 0, 0], // unknown → transparent
    }
}
