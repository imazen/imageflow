//TODO: not yet included in module tree

extern  crate rustface;
use internal_prelude::works_everywhere::*;
use ::std::option::Option;
use ::std::cmp;
use ::std::option::Option::*;
use num::Integer;

use imageflow_types::PixelFormat;
use self::rustface::{Detector, FaceInfo, ImageData};
use ::Context;

pub fn detect_faces(c: &Context, b: &BitmapBgra, model_bytes: Vec<u8>) -> Vec<FaceInfo>{
    let gray_bitmap = if b.fmt != PixelFormat::Gray8{
        let mut gray_bitmap = unsafe{ &mut *BitmapBgra::create(c, b.w, b.h, PixelFormat::Gray8, s::Color::Transparent).unwrap() };
        ::graphics::whitespace::approximate_grayscale(unsafe{ gray_bitmap.pixels_slice_mut().unwrap() }, gray_bitmap.stride(), 0,0,b.w, b.h, b);
        gray_bitmap //TODO: cleanup this gray bitmap early
    } else{
        b
    };
    let model = rustface::read_model(model_bytes).expect("Model must be valid");

    let mut detector = rustface::create_detector_with_model(model);

    detector.set_min_face_size(20);
    //TODO: set max face size
    detector.set_score_thresh(2.0);
    detector.set_pyramid_scale_factor(0.8);
    detector.set_slide_window_step(4, 4);

    let mut image = ImageData::new(gray_bitmap.pixels, gray_bitmap.w, gray_bitmap.h);
    detector.detect(&mut image)
}
