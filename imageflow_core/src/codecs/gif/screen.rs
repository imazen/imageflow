// Taken from https://github.com/kornelski/image-gif-dispose (MIT/Apache dual license)

use gif;

use super::disposal::Disposal;
use rgb::*;
use super::subimage::Subimage;
use super::bgra::BGRA8;
use std::io;
use std::error::Error;

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
    pub fn new<T: io::Read>(reader: &gif::Reader<T>) -> Self {
        let pal = reader.global_palette().map(|palette_bytes| to_bgra(palette_bytes));

        let pixels = reader.width() as usize * reader.height() as usize;
        let bg_color = if let (Some(bg_index), Some(pal)) = (reader.bg_color(), pal.as_ref()) {
            pal[bg_index]
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
    #[cfg_attr(feature = "cargo-clippy", allow(or_fun_call))]
    pub fn blit(&mut self, frame: &gif::Frame, buffer: &[u8]) -> Result<(), Box<dyn Error>> {
        let local_pal : Option<Vec<_>> = frame.palette.as_ref().map(|bytes| to_bgra(bytes));
        let pal = local_pal.as_ref().or(self.global_pal.as_ref()).ok_or("the frame must have _some_ palette")?;

        self.disposal.dispose(&mut self.pixels, self.width, self.bg_color);
        self.disposal = Disposal::new(frame, &self.pixels, self.width);

        for (dst, &src) in self.pixels.iter_mut().subimage(frame.left as usize, frame.top as usize, frame.width as usize, frame.height as usize, self.width).zip(buffer.iter()) {
            if let Some(transparent) = frame.transparent {
                if src == transparent {
                    continue;
                }
            }
            *dst = pal[src as usize];
        }

        Ok(())
    }
}

fn to_bgra(palette_bytes: &[u8]) -> Vec<BGRA8> {
    palette_bytes.chunks(3).map(|byte| BGRA8{r:byte[0], g:byte[1], b:byte[2], a:255}).collect()
}
