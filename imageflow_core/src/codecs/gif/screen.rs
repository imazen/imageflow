// Taken from https://github.com/kornelski/image-gif-dispose (MIT/Apache dual license)

use super::bgra::BGRA8;
use super::disposal::Disposal;
use super::subimage::Subimage;
use rgb::*;
use std::error::Error;
use std::io;

pub struct Screen {
    /// Result of combining frames
    pub pixels: Vec<BGRA8>,

    /// Width of the screen
    pub width: usize,

    /// Height of the screen
    pub height: usize,

    global_pal: Option<Vec<BGRA8>>,
    bg_color: BGRA8,
    disposal: Disposal<BGRA8>,
}

impl Screen {
    /// Initialize empty screen from GIF Reader.
    /// Make sure Reader is set to use Indexed color.
    /// `decoder.set(gif::ColorOutput::Indexed);`
    pub fn new<T: io::Read>(reader: &gif::Decoder<T>) -> Self {
        let pal = reader.global_palette().map(to_bgra);

        let pixels = reader.width() as usize * reader.height() as usize;
        let bg_color = if let (Some(bg_index), Some(pal)) = (reader.bg_color(), pal.as_ref()) {
            pal.get(bg_index).copied().unwrap_or_default()
        } else {
            BGRA8::default()
        };

        Screen {
            pixels: vec![bg_color; pixels],
            width: reader.width() as usize,
            height: reader.height() as usize,
            global_pal: pal,
            bg_color,
            disposal: Disposal::default(),
        }
    }

    /// Advance the screen by one frame.
    /// The result will be in `screen.pixels`
    //#[cfg_attr(feature = "cargo-clippy", allow(or_fun_call))]
    pub fn blit(&mut self, frame: &gif::Frame, buffer: &[u8]) -> Result<(), Box<dyn Error>> {
        let local_pal: Option<Vec<_>> = frame.palette.as_ref().map(|bytes| to_bgra(bytes));
        let pal = local_pal
            .as_ref()
            .or(self.global_pal.as_ref())
            .ok_or("the frame must have _some_ palette")?;

        // Clip frame bounds to canvas (matches browser behavior for malformed GIFs).
        let left = (frame.left as usize).min(self.width);
        let top = (frame.top as usize).min(self.height);
        let sub_w = (frame.width as usize).min(self.width.saturating_sub(left));
        let sub_h = (frame.height as usize).min(self.height.saturating_sub(top));

        self.disposal.dispose(&mut self.pixels, self.width, self.bg_color);
        self.disposal = Disposal::new(frame, &self.pixels, self.width, self.height);

        if sub_w == 0 || sub_h == 0 {
            return Ok(());
        }

        // Offset into the frame's pixel buffer to account for clipping.
        let frame_left_clip = left.saturating_sub(frame.left as usize);
        let frame_top_clip = top.saturating_sub(frame.top as usize);

        for (dst, &src) in self.pixels.iter_mut().subimage(left, top, sub_w, sub_h, self.width).zip(
            buffer.iter().subimage(
                frame_left_clip,
                frame_top_clip,
                sub_w,
                sub_h,
                frame.width as usize,
            ),
        ) {
            if let Some(transparent) = frame.transparent {
                if src == transparent {
                    continue;
                }
            }
            *dst = pal.get(src as usize).copied().unwrap_or(BGRA8 { r: 0, g: 0, b: 0, a: 0 });
        }

        Ok(())
    }
}

fn to_bgra(palette_bytes: &[u8]) -> Vec<BGRA8> {
    palette_bytes
        .chunks(3)
        .map(|byte| BGRA8 { r: byte[0], g: byte[1], b: byte[2], a: 255 })
        .collect()
}
