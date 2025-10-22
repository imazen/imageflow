use crate::graphics::prelude::*;
use imageflow_types::{Color, RoundCornersMode};
use rgb::alt::BGRA8;

fn get_radius(radius: RoundCornersMode, w: u32, h: u32) -> RoundCornersRadius {
    let smallest_dimension = w.min(h) as f32;
    match radius {
        RoundCornersMode::Percentage(p) => {
            RoundCornersRadius::All(smallest_dimension * p.min(100f32).max(0f32) / 200f32)
        }
        RoundCornersMode::Pixels(p) => {
            RoundCornersRadius::All(p.max(0f32).min(smallest_dimension / 2f32))
        }
        RoundCornersMode::Circle => RoundCornersRadius::Circle,
        RoundCornersMode::PercentageCustom { top_left, top_right, bottom_right, bottom_left } => {
            RoundCornersRadius::Custom([
                smallest_dimension * top_left.min(100f32).max(0f32) / 200f32,
                smallest_dimension * top_right.min(100f32).max(0f32) / 200f32,
                smallest_dimension * bottom_left.min(100f32).max(0f32) / 200f32,
                smallest_dimension * bottom_right.min(100f32).max(0f32) / 200f32,
            ])
        }
        RoundCornersMode::PixelsCustom { top_left, top_right, bottom_right, bottom_left } => {
            RoundCornersRadius::Custom([
                top_left.max(0f32).min(smallest_dimension / 2f32),
                top_right.max(0f32).min(smallest_dimension / 2f32),
                bottom_left.max(0f32).min(smallest_dimension / 2f32),
                bottom_right.max(0f32).min(smallest_dimension / 2f32),
            ])
        }
    }
}
#[derive(Copy, Clone, PartialEq, Debug)]
enum RoundCornersRadius {
    All(f32),
    Circle,
    Custom([f32; 4]),
}

fn plan_quadrants(
    radii: RoundCornersRadius,
    w: u32,
    h: u32,
) -> Result<[QuadrantInfo; 4], FlowError> {
    // Simplify Circle scenario
    if radii == RoundCornersRadius::Circle {
        let smallest_dimension = w.min(h) as f32;
        let offset_x = ((w as i64 - (h as i64)).max(0) / 2) as u32;
        let offset_y = ((h as i64 - (w as i64)).max(0) / 2) as u32;
        let mut quadrants =
            plan_quadrants(RoundCornersRadius::All(smallest_dimension / 2f32), w.min(h), w.min(h))
                .map_err(|e| e.at(here!()))?;
        for q in quadrants.iter_mut() {
            q.x += offset_x;
            q.y += offset_y;
            q.image_width = w;
            q.image_height = h;
            q.center_x += offset_x as f32;
            q.center_y += offset_y as f32;
        }
        return Ok(quadrants);
    }
    // Expand 'all' into corners
    if let RoundCornersRadius::All(v) = radii {
        return plan_quadrants(RoundCornersRadius::Custom([v, v, v, v]), w, h)
            .map_err(|e| e.at(here!()));
    }
    // Ok, deal with radius pixels
    if let RoundCornersRadius::Custom([top_left, top_right, bottom_left, bottom_right]) = radii {
        // Integer division so we don't overlap quadrants when dimensions are odd numbers
        let right_half_width = w / 2;
        let bottom_half_height = h / 2;
        let left_half_width = w - right_half_width;
        let top_half_height = h - bottom_half_height;

        Ok([
            QuadrantInfo {
                which: Quadrant::TopLeft,
                x: 0,
                y: 0,
                width: left_half_width,
                height: top_half_height,
                image_width: w,
                image_height: h,
                radius: top_left,
                center_x: top_left,
                center_y: top_left,
                is_top: true,
                is_left: true,
            },
            QuadrantInfo {
                which: Quadrant::TopRight,
                x: left_half_width,
                y: 0,
                width: right_half_width,
                height: top_half_height,
                image_width: w,
                image_height: h,
                radius: top_right,
                center_x: w as f32 - top_right,
                center_y: top_right,
                is_top: true,
                is_left: false,
            },
            QuadrantInfo {
                which: Quadrant::BottomLeft,
                x: 0,
                y: top_half_height,
                width: left_half_width,
                height: bottom_half_height,
                image_width: w,
                image_height: h,
                radius: bottom_left,
                center_x: bottom_left,
                center_y: h as f32 - bottom_left,
                is_top: false,
                is_left: true,
            },
            QuadrantInfo {
                which: Quadrant::BottomRight,
                x: left_half_width,
                y: top_half_height,
                width: right_half_width,
                height: bottom_half_height,
                image_width: w,
                image_height: h,
                radius: bottom_right,
                center_x: w as f32 - bottom_right,
                center_y: h as f32 - bottom_right,
                is_top: false,
                is_left: false,
            },
        ])
    } else {
        Err(unimpl!("Enum not handled, must be new"))
    }
}
#[derive(Copy, Clone, PartialEq, Debug)]
enum Quadrant {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}
#[derive(Copy, Clone, PartialEq, Debug)]
struct QuadrantInfo {
    which: Quadrant,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    image_width: u32,
    image_height: u32,
    radius: f32,
    center_x: f32,
    center_y: f32,
    is_top: bool,
    is_left: bool,
}

impl QuadrantInfo {
    fn bottom(&self) -> u32 {
        self.y + self.height
    }
    fn right(&self) -> u32 {
        self.x + self.width
    }
}

//
// fn plan_quadrant(center_x: f32, center_y: f32,
//                  radius: f32,
//                  canvas_width: u32,
//                  canvas_height: u32) -> Result<Vec<QuadrantDrawOp>, FlowError>{
//
//     let mut orders = Vec::with_capacity(radius.ceil() * 8);
//     let r2f = radius * radius;
//
//
//
//     for y in (0..=radius_ceil).rev(){
//         let yf = y as f32 - 0.5;
//         clear_widths.push(radius_ceil - f32::sqrt(r2f - yf * yf).round() as usize);
//     }
// }

pub fn flow_bitmap_bgra_clear_around_rounded_corners(
    b: &mut BitmapWindowMut<BGRA8>,
    round_corners_mode: RoundCornersMode,
    color: Color,
) -> Result<(), FlowError> {
    if b.info().pixel_layout() != PixelLayout::BGRA {
        return Err(nerror!(ErrorKind::InvalidArgument, "Only BGRA supported for rounded corners"));
    }

    let colorcontext = ColorContext::new(WorkingFloatspace::LinearRGB, 0f32);
    let matte32 = color.to_color_32().map_err(|e| FlowError::from(e).at(here!()))?;
    let matte = matte32.to_bgra8();

    let alpha_to_float = (1.0f32) / 255.0f32;

    let matte_a = matte.a as f32 * alpha_to_float;
    let matte_b = colorcontext.srgb_to_floatspace(matte.b);
    let matte_g = colorcontext.srgb_to_floatspace(matte.g);
    let matte_r = colorcontext.srgb_to_floatspace(matte.r);

    let w = b.w();
    let h = b.h();

    //If you created a circle with the surface area of a 1x1 square, this would be its radius
    //Useful for calculating pixel intensities while being correct on average regardless of angle
    let volumetric_offset = 0.56419f32;

    let radius_set = get_radius(round_corners_mode, b.w(), b.h());
    let quadrants = plan_quadrants(radius_set, b.w(), b.h()).map_err(|e| e.at(here!()))?;

    for quadrant in quadrants {
        if quadrant.y > 0 && quadrant.which == Quadrant::TopLeft {
            //Clear top rows, must be a circle
            b.fill_rectangle(matte32, 0, 0, w, quadrant.y).map_err(|e| e.at(here!()))?;
        }
        if h > quadrant.bottom() && quadrant.which == Quadrant::BottomLeft {
            //Clear bottom rows, must be a circle
            b.fill_rectangle(matte32, 0, quadrant.bottom(), w, h).map_err(|e| e.at(here!()))?;
        }
        let radius_ceil = quadrant.radius.ceil() as usize;
        let start_y = if quadrant.is_top {
            quadrant.y as usize
        } else {
            quadrant.bottom() as usize - radius_ceil
        };
        let end_y = if quadrant.is_top {
            quadrant.y as usize + radius_ceil
        } else {
            quadrant.bottom() as usize
        };
        let start_x = if quadrant.is_left {
            quadrant.x as usize
        } else {
            quadrant.right() as usize - radius_ceil
        };
        let end_x = if quadrant.is_left {
            quadrant.x as usize + radius_ceil
        } else {
            quadrant.right() as usize
        };

        let (clear_x_from, clear_x_to) =
            if quadrant.is_left { (0, quadrant.x) } else { (quadrant.right(), w) };

        //Clear the edges for rows where the quadrant isn't rendering an arc
        if clear_x_from != clear_x_to {
            for y in (quadrant.y..start_y as u32).chain(end_y as u32..quadrant.bottom()) {
                b.fill_rectangle(matte32, clear_x_from, y, clear_x_to, y + 1)
                    .map_err(|e| e.at(here!()))?;
            }
        }

        // Calculate radii
        // Pixels within the radius of solid are never touched
        // Pixels within the radius of influence may be aliased
        // Pixels outside the radius of influence are replaced with the matte
        let radius_of_influence = quadrant.radius + (1f32 - volumetric_offset);
        let radius_of_solid = quadrant.radius - volumetric_offset;
        let radius_aliasing_width = radius_of_influence - radius_of_solid;

        let radius_of_influence_squared = radius_of_influence * radius_of_influence;
        let radius_of_solid_squared = radius_of_solid * radius_of_solid;

        for y in start_y..end_y {
            let mut row_window = b.row_window(y as u32).unwrap();
            let row_pixels = row_window.slice_mut();
            let yf = y as f32 + 0.5;
            let y_dist_from_center = (quadrant.center_y - yf).abs();
            let y_dist_squared = y_dist_from_center * y_dist_from_center;

            let x_dist_from_center_solid =
                f32::sqrt((radius_of_solid_squared - y_dist_squared).max(0f32));
            let x_dist_from_center_influenced =
                f32::sqrt((radius_of_influence_squared - y_dist_squared).max(0f32));

            let edge_solid_x1 =
                (quadrant.center_x - x_dist_from_center_solid).ceil().max(0f32) as usize;
            let edge_solid_x2 =
                (quadrant.center_x + x_dist_from_center_solid).floor().min(w as f32) as usize;

            let edge_influence_x1 =
                (quadrant.center_x - x_dist_from_center_influenced).floor().max(0f32) as usize;
            let edge_influence_x2 =
                (quadrant.center_x + x_dist_from_center_influenced).ceil().min(w as f32) as usize;

            //Clear what we don't need to alias
            if quadrant.is_left {
                row_pixels[0..edge_influence_x1].fill(matte);
            } else {
                row_pixels[edge_influence_x2..w as usize].fill(matte);
            };

            let (alias_from, alias_to) = if quadrant.is_left {
                (edge_influence_x1, edge_solid_x1)
            } else {
                (edge_solid_x2, edge_influence_x2)
            };

            for x in alias_from..alias_to {
                let xf = x as f32 + 0.5;
                let diff_x = quadrant.center_x - xf;
                let distance = (diff_x * diff_x + y_dist_squared).sqrt();

                if distance > radius_of_influence {
                    row_pixels[x] = matte;
                } else if distance > radius_of_solid {
                    //Intensity should be 0..1, where 1 is full matte color and 0 is full image color
                    let intensity = (distance - radius_of_solid) / (radius_aliasing_width);

                    let pixel = row_pixels[x];
                    let pixel_a = pixel.a;
                    let pixel_a_f32 = pixel_a as i32 as f32 * alpha_to_float * (1f32 - intensity);

                    let matte_a = (1.0f32 - pixel_a_f32) * matte_a;
                    let final_a: f32 = matte_a + pixel_a_f32;
                    row_pixels[x] = rgb::alt::BGRA8 {
                        b: colorcontext.floatspace_to_srgb(
                            (colorcontext.srgb_to_floatspace(pixel.b) * pixel_a_f32
                                + matte_b * matte_a)
                                / final_a,
                        ),
                        g: colorcontext.floatspace_to_srgb(
                            (colorcontext.srgb_to_floatspace(pixel.g) * pixel_a_f32
                                + matte_g * matte_a)
                                / final_a,
                        ),
                        r: colorcontext.floatspace_to_srgb(
                            (colorcontext.srgb_to_floatspace(pixel.r) * pixel_a_f32
                                + matte_r * matte_a)
                                / final_a,
                        ),
                        a: uchar_clamp_ff(255f32 * final_a),
                    };
                }
            }
        }
    }
    Ok(())
}
