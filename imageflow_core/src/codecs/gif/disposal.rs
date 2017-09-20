// Taken from https://github.com/pornel/image-gif-dispose (MIT/Apache dual license)

use gif;
use gif::DisposalMethod::*;
use super::subimage::Subimage;
use std::default::Default;

pub struct Disposal<Pixel: Copy> {
    method: gif::DisposalMethod,
    previous_pixels: Option<Vec<Pixel>>,
    left: u16, top: u16,
    width: u16, height: u16,
}

impl<Pixel: Copy> Default for Disposal<Pixel> {
    fn default() -> Self {
        Disposal {
            method: gif::DisposalMethod::Keep,
            previous_pixels: None,
            top: 0, left: 0, width: 0, height: 0,
        }
    }
}

impl<Pixel: Copy> Disposal<Pixel> {
    pub fn dispose(&mut self, pixels: &mut [Pixel], stride: usize, bg_color: Pixel) {
        if self.width == 0 || self.height == 0 {
            return;
        }

        let pixels_iter = pixels.iter_mut().subimage(self.left as usize, self.top as usize, self.width as usize, self.height as usize, stride);
        match self.method {
            Background => for px in pixels_iter { *px = bg_color; },
            Previous => if let Some(saved) = self.previous_pixels.take() {
                for (px, &src) in pixels_iter.zip(saved.iter()) { *px = src; }
            },
            Keep | Any => {},
        }
    }

    pub fn new(frame: &gif::Frame, pixels: &[Pixel], stride: usize) -> Self {
        Disposal {
            method: frame.dispose,
            left: frame.left,
            top: frame.top,
            width: frame.width,
            height: frame.height,
            previous_pixels: match frame.dispose {
                Previous => Some(pixels.iter().cloned().subimage(frame.left as usize, frame.top as usize, frame.width as usize, frame.height as usize, stride).collect()),
                _ => None,
            },
        }
    }
}
