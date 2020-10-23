use crate::ffi::{BitmapFloat,BitmapBgra,BitmapCompositingMode};
use crate::imaging::weights::*;
use crate::FlowError;

#[cfg(target_arch = "x86")]
pub use std::arch::x86::{
    __m128, _mm_add_ps, _mm_loadu_ps, _mm_movehl_ps, _mm_movelh_ps, _mm_mul_ps, _mm_set1_ps,
    _mm_setr_ps, _mm_setzero_ps, _mm_storeu_ps, _mm_unpackhi_ps, _mm_unpacklo_ps,
};
#[cfg(target_arch = "x86_64")]
pub use std::arch::x86_64::{
    __m128, _mm_add_ps, _mm_loadu_ps, _mm_movehl_ps, _mm_movelh_ps, _mm_mul_ps, _mm_set1_ps,
    _mm_setr_ps, _mm_setzero_ps, _mm_storeu_ps, _mm_unpackhi_ps, _mm_unpacklo_ps,
};


#[no_mangle]
pub unsafe fn flow_bitmap_float_scale_rows(
    from: &BitmapFloat,
    from_row: u32,
    to: &mut BitmapFloat,
    to_row: u32,
    row_count: u32,
    weights: &PixelRowWeights
) -> Result<(), FlowError> {
    let from_step: u32 = from.channels;
    let to_step: u32 = to.channels;
    let dest_buffer_count: u32 = to.w;
    let min_channels: u32 = from_step.min(to_step);
    let mut ndx: u32 = 0;
    if min_channels > 4 as i32 as u32 {
        return Err(nerror!(ErrorKind::InvalidInternalState));
    }
    let mut avg: [f32; 4] = [0.; 4];
    // if both have alpha, process it
    if from_step == 4 && to_step == 4 {
        let mut row: u32 = 0;
        while row < row_count {
            let source_offset = ((from_row + row) * (*from).float_stride) as isize;
            let source_buffer: *const __m128 =
                (*from).pixels.offset(source_offset) as *const __m128;
            let dest_offset = ((to_row + row) * (*to).float_stride) as isize;
            let dest_buffer: *mut __m128 = (*to).pixels.offset(dest_offset) as *mut __m128;
            let dest_buffer: &mut [__m128] =
                std::slice::from_raw_parts_mut(dest_buffer, dest_buffer_count as usize);
            ndx = 0;
            while ndx < dest_buffer_count {
                let mut sums: __m128 = _mm_set1_ps(0.0);
                let left: i32 = (*weights.offset(ndx as isize)).Left;
                let right: i32 = (*weights.offset(ndx as isize)).Right;
                let weightArray: *const f32 = (*weights.offset(ndx as isize)).Weights;
                let source_buffer: &[__m128] =
                    std::slice::from_raw_parts(source_buffer, (right + 1) as usize);
                /* Accumulate each channel */
                let mut i = left;
                while i <= right {
                    let factor: __m128 = _mm_set1_ps(*weightArray.offset((i - left) as isize));
                    // sums += factor * *source_buffer[i as usize];
                    let mid = _mm_mul_ps(factor, source_buffer[i as usize]);
                    sums = _mm_add_ps(sums, mid);
                    i += 1
                }
                dest_buffer[ndx as usize] = sums;
                ndx += 1
            }
            row += 1
        }
    } else if from_step == 3 as i32 as u32 && to_step == 3 as i32 as u32 {
        let mut row_0: u32 = 0 as i32 as u32;
        while row_0 < row_count {
            let source_buffer_0: *const f32 = (*from).pixels.offset(
                from_row
                    .wrapping_add(row_0)
                    .wrapping_mul((*from).float_stride) as isize,
            );
            let dest_buffer_0: *mut f32 = (*to)
                .pixels
                .offset(to_row.wrapping_add(row_0).wrapping_mul((*to).float_stride) as isize);
            ndx = 0 as i32 as u32;
            while ndx < dest_buffer_count {
                let mut bgr: [f32; 3] = [0.0f32, 0.0f32, 0.0f32];
                let left_0: i32 = (*weights.offset(ndx as isize)).Left;
                let right_0: i32 = (*weights.offset(ndx as isize)).Right;
                let weightArray_0: *const f32 = (*weights.offset(ndx as isize)).Weights;
                let mut i_0: i32 = 0;
                /* Accumulate each channel */
                i_0 = left_0;
                while i_0 <= right_0 {
                    let weight: f32 = *weightArray_0.offset((i_0 - left_0) as isize);
                    bgr[0] += weight
                        * *source_buffer_0.offset((i_0 as u32).wrapping_mul(from_step) as isize);
                    bgr[1] += weight
                        * *source_buffer_0.offset(
                        (i_0 as u32).wrapping_mul(from_step).wrapping_add(1u32) as isize,
                    );
                    bgr[2] += weight
                        * *source_buffer_0.offset(
                        (i_0 as u32).wrapping_mul(from_step).wrapping_add(2u32) as isize,
                    );
                    i_0 += 1
                }
                *dest_buffer_0.offset(ndx.wrapping_mul(to_step) as isize) = bgr[0];
                *dest_buffer_0.offset(ndx.wrapping_mul(to_step).wrapping_add(1u32) as isize) =
                    bgr[1];
                *dest_buffer_0.offset(ndx.wrapping_mul(to_step).wrapping_add(2u32) as isize) =
                    bgr[2];
                ndx = ndx.wrapping_add(1)
            }
            row_0 = row_0.wrapping_add(1)
        }
    } else {
        let mut row_1: u32 = 0 as i32 as u32;
        while row_1 < row_count {
            let source_buffer_1: *const f32 = (*from).pixels.offset(
                from_row
                    .wrapping_add(row_1)
                    .wrapping_mul((*from).float_stride) as isize,
            );
            let dest_buffer_1: *mut f32 = (*to)
                .pixels
                .offset(to_row.wrapping_add(row_1).wrapping_mul((*to).float_stride) as isize);
            ndx = 0 as i32 as u32;
            while ndx < dest_buffer_count {
                avg[0] = 0 as i32 as f32;
                avg[1] = 0 as i32 as f32;
                avg[2] = 0 as i32 as f32;
                avg[3 as i32 as usize] = 0 as i32 as f32;
                let left_1: i32 = (*weights.offset(ndx as isize)).Left;
                let right_1: i32 = (*weights.offset(ndx as isize)).Right;
                let weightArray_1: *const f32 = (*weights.offset(ndx as isize)).Weights;
                /* Accumulate each channel */
                let mut i_1: i32 = left_1;
                while i_1 <= right_1 {
                    let weight_0: f32 = *weightArray_1.offset((i_1 - left_1) as isize);
                    let mut j: u32 = 0 as i32 as u32;
                    while j < min_channels {
                        avg[j as usize] += weight_0
                            * *source_buffer_1.offset(
                            (i_1 as u32).wrapping_mul(from_step).wrapping_add(j) as isize,
                        );
                        j = j.wrapping_add(1)
                    }
                    i_1 += 1
                }
                let mut j_0: u32 = 0 as i32 as u32;
                while j_0 < min_channels {
                    *dest_buffer_1.offset(ndx.wrapping_mul(to_step).wrapping_add(j_0) as isize) =
                        avg[j_0 as usize];
                    j_0 = j_0.wrapping_add(1)
                }
                ndx = ndx.wrapping_add(1)
            }
            row_1 = row_1.wrapping_add(1)
        }
    }
    Ok(())
}
unsafe fn multiply_row(row: *mut f32, length: usize, coefficient: f32) {
    let mut i: usize = 0 as i32 as usize;
    while i < length {
        *row.offset(i as isize) *= coefficient;
        i = i.wrapping_add(1)
    }
}
unsafe fn add_row(mutate_row: *mut f32, input_row: *mut f32, length: usize) {
    let mut i: usize = 0 as i32 as usize;
    while i < length {
        *mutate_row.offset(i as isize) += *input_row.offset(i as isize);
        i = i.wrapping_add(1)
    }
}

unsafe extern "C" fn crop(
    c: *mut flow_c,
    b: *mut flow_bitmap_bgra,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> Result<*mut flow_bitmap_bgra,FlowError> {
    if h.wrapping_add(y) > (*b).h || w.wrapping_add(x) > (*b).w {
        return Err(nerror!(ErrorKind::InvalidArgument));
    }
    let mut cropped_canvas: *mut flow_bitmap_bgra = BitmapBgra::create()
        flow_bitmap_bgra_create_header(c, w as i32, h as i32);
    let bpp: u32 = flow_pixel_format_bytes_per_pixel((*b).fmt);
    if cropped_canvas.is_null() {
        flow_context_add_to_callstack(
            c,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            640 as i32,
            (*::std::mem::transmute::<&[u8; 5], &[libc::c_char; 5]>(b"crop\x00")).as_ptr(),
        );
        return NULL as *mut flow_bitmap_bgra;
    }
    (*cropped_canvas).fmt = (*b).fmt;
    memcpy(
        &mut *(*cropped_canvas).matte_color.as_mut_ptr().offset(0) as *mut u8 as *mut libc::c_void,
        &mut *(*b).matte_color.as_mut_ptr().offset(0) as *mut u8 as *const libc::c_void,
        ::std::mem::size_of::<[u8; 4]>() as u64,
    );
    (*cropped_canvas).compositing_mode = (*b).compositing_mode;
    (*cropped_canvas).pixels = (*b)
        .pixels
        .offset(y.wrapping_mul((*b).stride) as isize)
        .offset(x.wrapping_mul(bpp) as isize);
    (*cropped_canvas).stride = (*b).stride;
    return cropped_canvas;
}



#[no_mangle]
pub unsafe extern "C" fn flow_node_execute_scale2d_render1d(
    c: *mut flow_c,
    input: *mut flow_bitmap_bgra,
    uncropped_canvas: *mut flow_bitmap_bgra,
    info: *mut flow_nodeinfo_scale2d_render_to_canvas1d,
) -> Result<(),FlowError> {
    if (*info).h.wrapping_add((*info).y) > (*uncropped_canvas).h
        || (*info).w.wrapping_add((*info).x) > (*uncropped_canvas).w
    {
        return Err(nerror!(ErrorKind::InvalidArgument));
    }
    let cropped_canvas: *mut flow_bitmap_bgra = if (*info).x == 0
        && (*info).y == 0
        && (*info).w == (*uncropped_canvas).w
        && (*info).h == (*uncropped_canvas).h
    {
        uncropped_canvas
    } else {
        crop(
            c,
            uncropped_canvas,
            (*info).x,
            (*info).y,
            (*info).w,
            (*info).h,
        )
    };
    if cropped_canvas.is_null() {
        return Err(nerror!(ErrorKind::InvalidArgument));

        flow_context_add_to_callstack(
            c,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            665 as i32,
            (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                b"flow_node_execute_scale2d_render1d\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    let input_fmt: flow_pixel_format = flow_effective_pixel_format(input);
    let canvas_fmt: flow_pixel_format = flow_effective_pixel_format(cropped_canvas);
    if input_fmt as u32 != flow_bgra32 as i32 as u32 && input_fmt as u32 != flow_bgr32 as i32 as u32
    {
        FLOW_error(
            c,
            flow_status_code::Not_implemented,
            "flow_node_execute_scale2d_render1d",
        );
        return false;
    }
    if canvas_fmt as u32 != flow_bgra32 as i32 as u32
        && canvas_fmt as u32 != flow_bgr32 as i32 as u32
    {
        FLOW_error(
            c,
            flow_status_code::Not_implemented,
            "flow_node_execute_scale2d_render1d",
        );
        return false;
    }
    let mut colorcontext: flow_colorcontext_info = flow_colorcontext_info {
        byte_to_float: [0.; 256],
        floatspace: flow_working_floatspace_srgb,
        apply_srgb: false,
        apply_gamma: false,
        gamma: 0.,
        gamma_inverse: 0.,
    };
    flow_colorcontext_init(
        c,
        &mut colorcontext,
        (*info).scale_in_colorspace,
        0 as i32 as f32,
        0 as i32 as f32,
        0 as i32 as f32,
    );
    // Use details as a parent structure to ensure everything gets freed
    let mut details: *mut flow_interpolation_details =
        flow_interpolation_details_create_from(c, (*info).interpolation_filter);
    if details.is_null() {
        flow_context_add_to_callstack(
            c,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            686 as i32,
            (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                b"flow_node_execute_scale2d_render1d\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    (*details).sharpen_percent_goal = (*info).sharpen_percent_goal;
    let mut contrib_v: *mut flow_interpolation_line_contributions =
        NULL as *mut flow_interpolation_line_contributions;
    let mut contrib_h: *mut flow_interpolation_line_contributions =
        NULL as *mut flow_interpolation_line_contributions;
    flow_context_profiler_start(
        c,
        b"contributions_calc\x00" as *const u8 as *const libc::c_char,
        0 as i32 != 0,
    );
    contrib_v = flow_interpolation_line_contributions_create(c, (*info).h, (*input).h, details);
    if contrib_v.is_null()
        || !flow_set_owner(
        c,
        contrib_v as *mut libc::c_void,
        details as *mut libc::c_void,
    )
    {
        flow_destroy(
            c,
            details as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            697 as i32,
        );
        flow_context_add_to_callstack(
            c,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            698 as i32,
            (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                b"flow_node_execute_scale2d_render1d\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    contrib_h = flow_interpolation_line_contributions_create(c, (*info).w, (*input).w, details);
    if contrib_h.is_null()
        || !flow_set_owner(
        c,
        contrib_h as *mut libc::c_void,
        details as *mut libc::c_void,
    )
    {
        flow_destroy(
            c,
            details as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            702 as i32,
        );
        flow_context_add_to_callstack(
            c,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            703 as i32,
            (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                b"flow_node_execute_scale2d_render1d\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    flow_context_profiler_stop(
        c,
        b"contributions_calc\x00" as *const u8 as *const libc::c_char,
        1 as i32 != 0,
        0 as i32 != 0,
    );
    flow_context_profiler_start(
        c,
        b"create_bitmap_float (buffers)\x00" as *const u8 as *const libc::c_char,
        0 as i32 != 0,
    );
    let mut source_buf: *mut flow_bitmap_float =
        flow_bitmap_float_create_header(c, (*input).w as i32, 1 as i32, 4 as i32);
    if source_buf.is_null()
        || !flow_set_owner(
        c,
        source_buf as *mut libc::c_void,
        details as *mut libc::c_void,
    )
    {
        flow_destroy(
            c,
            details as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            711 as i32,
        );
        flow_context_add_to_callstack(
            c,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            712 as i32,
            (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                b"flow_node_execute_scale2d_render1d\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    let mut dest_buf: *mut flow_bitmap_float =
        flow_bitmap_float_create(c, (*info).w as i32, 1 as i32, 4 as i32, true);
    if dest_buf.is_null()
        || !flow_set_owner(
        c,
        dest_buf as *mut libc::c_void,
        details as *mut libc::c_void,
    )
    {
        flow_destroy(
            c,
            details as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            716 as i32,
        );
        flow_context_add_to_callstack(
            c,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            717 as i32,
            (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                b"flow_node_execute_scale2d_render1d\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    (*source_buf).alpha_meaningful = input_fmt as u32 == flow_bgra32 as i32 as u32;
    (*dest_buf).alpha_meaningful = (*source_buf).alpha_meaningful;
    (*source_buf).alpha_premultiplied = (*source_buf).channels == 4 as i32 as u32;
    (*dest_buf).alpha_premultiplied = (*source_buf).alpha_premultiplied;
    flow_context_profiler_stop(
        c,
        b"create_bitmap_float (buffers)\x00" as *const u8 as *const libc::c_char,
        1 as i32 != 0,
        0 as i32 != 0,
    );
    // Determine how many rows we need to buffer
    let mut max_input_rows: i32 = 0 as i32;
    let mut i: u32 = 0 as i32 as u32;
    while i < (*contrib_v).LineLength {
        let inputs: i32 = (*(*contrib_v).ContribRow.offset(i as isize)).Right
            - (*(*contrib_v).ContribRow.offset(i as isize)).Left
            + 1 as i32;
        if inputs > max_input_rows {
            max_input_rows = inputs
        }
        i = i.wrapping_add(1)
    }
    // Allocate space
    let row_floats: usize = (4u32).wrapping_mul((*input).w) as usize;
    let buf: *mut f32 = flow_context_malloc(
        c,
        ::std::mem::size_of::<f32>()
            .wrapping_mul(row_floats)
            .wrapping_mul((max_input_rows + 1) as usize),
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        details as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        737 as i32,
    ) as *mut f32;
    let rows: *mut *mut f32 = flow_context_malloc(
        c,
        (::std::mem::size_of::<*mut f32>()).wrapping_mul(max_input_rows as usize),
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        details as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        738 as i32,
    ) as *mut *mut f32;
    let row_coefficients: *mut f32 = flow_context_malloc(
        c,
        ::std::mem::size_of::<f32>().wrapping_mul(max_input_rows as usize),
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        details as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        739 as i32,
    ) as *mut f32;
    let row_indexes: *mut i32 = flow_context_malloc(
        c,
        ::std::mem::size_of::<i32>().wrapping_mul(max_input_rows as usize),
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        details as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        740 as i32,
    ) as *mut i32;
    if buf.is_null() || rows.is_null() || row_coefficients.is_null() || row_indexes.is_null() {
        flow_destroy(
            c,
            details as *mut libc::c_void,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            742 as i32,
        );
        flow_context_add_to_callstack(
            c,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            743 as i32,
            (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                b"flow_node_execute_scale2d_render1d\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    let output_address: *mut f32 =
        &mut *buf.offset(row_floats.wrapping_mul(max_input_rows as usize) as isize) as *mut f32;
    let mut i_0: i32 = 0 as i32;
    while i_0 < max_input_rows {
        let ref mut fresh8 = *rows.offset(i_0 as isize);
        *fresh8 = &mut *buf
            .offset((4u32).wrapping_mul((*input).w).wrapping_mul(i_0 as u32) as isize)
            as *mut f32;
        *row_coefficients.offset(i_0 as isize) = 1 as i32 as f32;
        *row_indexes.offset(i_0 as isize) = -(1 as i32);
        i_0 += 1
    }
    let mut out_row: u32 = 0 as i32 as u32;
    while out_row < (*cropped_canvas).h {
        let contrib: flow_interpolation_pixel_contributions =
            *(*contrib_v).ContribRow.offset(out_row as isize);

        // Clear output row
        ::libc::memset(
            output_address as *mut libc::c_void,
            0 as i32,
            ::std::mem::size_of::<f32>().wrapping_mul(row_floats),
        );
        let mut input_row: i32 = contrib.Left;
        while input_row <= contrib.Right {
            // Try to find row in buffer if already loaded
            let mut loaded: bool = false;
            let mut active_buf_ix: i32 = -(1 as i32);
            let mut buf_row: i32 = 0 as i32;
            while buf_row < max_input_rows {
                if *row_indexes.offset(buf_row as isize) == input_row {
                    active_buf_ix = buf_row;
                    loaded = true;
                    break;
                } else {
                    buf_row += 1
                }
            }
            // Not loaded?
            if !loaded {
                let mut buf_row_0: i32 = 0 as i32; // Buffer too small!
                while buf_row_0 < max_input_rows {
                    if *row_indexes.offset(buf_row_0 as isize) < contrib.Left {
                        active_buf_ix = buf_row_0;
                        loaded = false;
                        break;
                    } else {
                        buf_row_0 += 1
                    }
                }
            }
            if active_buf_ix < 0 as i32 {
                flow_destroy(
                    c,
                    details as *mut libc::c_void,
                    b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                    779 as i32,
                );
                flow_context_set_error_get_message_buffer(
                    c,
                    flow_status_code::Invalid_internal_state,
                    b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                    780 as i32,
                    (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                        b"flow_node_execute_scale2d_render1d\x00",
                    ))
                        .as_ptr(),
                );
                return false;
            }
            if !loaded {
                // Load row
                (*source_buf).pixels = *rows.offset(active_buf_ix as isize);
                flow_context_profiler_start(
                    c,
                    b"convert_srgb_to_linear\x00" as *const u8 as *const libc::c_char,
                    0 as i32 != 0,
                );
                if !flow_bitmap_float_convert_srgb_to_linear(
                    c,
                    &mut colorcontext,
                    input,
                    input_row as u32,
                    source_buf,
                    0 as i32 as u32,
                    1 as i32 as u32,
                ) {
                    flow_destroy(
                        c,
                        details as *mut libc::c_void,
                        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                        789 as i32,
                    );
                    flow_context_add_to_callstack(
                        c,
                        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                        790 as i32,
                        (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                            b"flow_node_execute_scale2d_render1d\x00",
                        ))
                            .as_ptr(),
                    );
                    return false;
                }
                flow_context_profiler_stop(
                    c,
                    b"convert_srgb_to_linear\x00" as *const u8 as *const libc::c_char,
                    1 as i32 != 0,
                    0 as i32 != 0,
                );
                *row_coefficients.offset(active_buf_ix as isize) = 1 as i32 as f32;
                *row_indexes.offset(active_buf_ix as isize) = input_row;
                loaded = true
            }
            let weight: f32 = *contrib.Weights.offset((input_row - contrib.Left) as isize);
            if fabs(weight as f64) > 0.00000002f64 {
                // Apply coefficient, update tracking
                let delta_coefficient: f32 =
                    weight / *row_coefficients.offset(active_buf_ix as isize);
                multiply_row(
                    *rows.offset(active_buf_ix as isize),
                    row_floats,
                    delta_coefficient,
                );
                *row_coefficients.offset(active_buf_ix as isize) = weight;
                // Add row
                add_row(
                    output_address,
                    *rows.offset(active_buf_ix as isize),
                    row_floats,
                );
            }
            input_row += 1
        }
        // The container now points to the row which has been vertically scaled
        (*source_buf).pixels = output_address;
        // Now scale horizontally!
        flow_context_profiler_start(
            c,
            b"ScaleBgraFloatRows\x00" as *const u8 as *const libc::c_char,
            0 as i32 != 0,
        );
        if !flow_bitmap_float_scale_rows(
            c,
            source_buf,
            0 as i32 as u32,
            dest_buf,
            0 as i32 as u32,
            1 as i32 as u32,
            (*contrib_h).ContribRow,
        ) {
            flow_destroy(
                c,
                details as *mut libc::c_void,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                816 as i32,
            );
            flow_context_add_to_callstack(
                c,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                817 as i32,
                (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                    b"flow_node_execute_scale2d_render1d\x00",
                ))
                    .as_ptr(),
            );
            return false;
        }
        flow_context_profiler_stop(
            c,
            b"ScaleBgraFloatRows\x00" as *const u8 as *const libc::c_char,
            1 as i32 != 0,
            0 as i32 != 0,
        );
        if !flow_bitmap_float_composite_linear_over_srgb(
            c,
            &mut colorcontext,
            dest_buf,
            0 as i32 as u32,
            cropped_canvas,
            out_row,
            1 as i32 as u32,
            false,
        ) {
            flow_destroy(
                c,
                details as *mut libc::c_void,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                822 as i32,
            );
            flow_context_add_to_callstack(
                c,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                823 as i32,
                (*::std::mem::transmute::<&[u8; 35], &[libc::c_char; 35]>(
                    b"flow_node_execute_scale2d_render1d\x00",
                ))
                    .as_ptr(),
            );
            return false;
        }
        out_row = out_row.wrapping_add(1)
    }
    flow_destroy(
        c,
        if cropped_canvas == uncropped_canvas {
            0 as *mut flow_bitmap_bgra
        } else {
            cropped_canvas
        } as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        826 as i32,
    );
    flow_destroy(
        c,
        details as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        827 as i32,
    );
    return true;
}