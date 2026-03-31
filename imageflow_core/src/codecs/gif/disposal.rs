// Taken from https://github.com/kornelski/image-gif-dispose (MIT/Apache dual license)

use super::subimage::Subimage;
use gif::DisposalMethod::*;
use std::default::Default;

pub struct Disposal<Pixel: Copy> {
    method: gif::DisposalMethod,
    previous_pixels: Option<Vec<Pixel>>,
    left: u16,
    top: u16,
    width: u16,
    height: u16,
}

impl<Pixel: Copy> Default for Disposal<Pixel> {
    fn default() -> Self {
        Disposal {
            method: gif::DisposalMethod::Keep,
            previous_pixels: None,
            top: 0,
            left: 0,
            width: 0,
            height: 0,
        }
    }
}

impl<Pixel: Copy> Disposal<Pixel> {
    pub fn dispose(&mut self, pixels: &mut [Pixel], stride: usize, bg_color: Pixel) {
        let (w, h, l, t) = (
            self.width as usize,
            self.height as usize,
            self.left as usize,
            self.top as usize,
        );
        if w == 0 || h == 0 || l.saturating_add(w) > stride {
            return;
        }

        let pixels_iter = pixels.iter_mut().subimage(l, t, w, h, stride);
        match self.method {
            Background => {
                for px in pixels_iter {
                    *px = bg_color;
                }
            }
            Previous => {
                if let Some(saved) = self.previous_pixels.take() {
                    for (px, &src) in pixels_iter.zip(saved.iter()) {
                        *px = src;
                    }
                }
            }
            Keep | Any => {}
        }
    }

    pub fn new(frame: &gif::Frame, pixels: &[Pixel], stride: usize, canvas_height: usize) -> Self {
        // Clip frame bounds to canvas dimensions.
        let l = (frame.left as usize).min(stride);
        let t = (frame.top as usize).min(canvas_height);
        let w = (frame.width as usize).min(stride.saturating_sub(l));
        let h = (frame.height as usize).min(canvas_height.saturating_sub(t));
        let valid = w > 0 && h > 0;

        Disposal {
            method: frame.dispose,
            left: l as u16,
            top: t as u16,
            width: w as u16,
            height: h as u16,
            previous_pixels: match frame.dispose {
                Previous if valid => Some(
                    pixels
                        .iter()
                        .cloned()
                        .subimage(l, t, w, h, stride)
                        .collect(),
                ),
                _ => None,
            },
        }
    }
}
