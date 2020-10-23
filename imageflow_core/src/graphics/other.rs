#[inline]
unsafe extern "C" fn transpose4x4_SSE(A: *mut f32, B: *mut f32, lda: i32, ldb: i32) {
    let mut row1: __m128 = _mm_loadu_ps(&mut *A.offset((0 as i32 * lda) as isize));
    let mut row2: __m128 = _mm_loadu_ps(&mut *A.offset((1 as i32 * lda) as isize));
    let mut row3: __m128 = _mm_loadu_ps(&mut *A.offset((2 as i32 * lda) as isize));
    let mut row4: __m128 = _mm_loadu_ps(&mut *A.offset((3 as i32 * lda) as isize));
    let mut tmp3: __m128 = _mm_setzero_ps();
    let mut tmp2: __m128 = _mm_setzero_ps();
    let mut tmp1: __m128 = _mm_setzero_ps();
    let mut tmp0: __m128 = _mm_setzero_ps();
    tmp0 = _mm_unpacklo_ps(row1, row2);
    tmp2 = _mm_unpacklo_ps(row3, row4);
    tmp1 = _mm_unpackhi_ps(row1, row2);
    tmp3 = _mm_unpackhi_ps(row3, row4);
    row1 = _mm_movelh_ps(tmp0, tmp2);
    row2 = _mm_movehl_ps(tmp2, tmp0);
    row3 = _mm_movelh_ps(tmp1, tmp3);
    row4 = _mm_movehl_ps(tmp3, tmp1);
    _mm_storeu_ps(&mut *B.offset((0 as i32 * ldb) as isize), row1);
    _mm_storeu_ps(&mut *B.offset((1 as i32 * ldb) as isize), row2);
    _mm_storeu_ps(&mut *B.offset((2 as i32 * ldb) as isize), row3);
    _mm_storeu_ps(&mut *B.offset((3 as i32 * ldb) as isize), row4);
}
#[inline]
unsafe extern "C" fn transpose_block_SSE4x4(
    A: *mut f32,
    B: *mut f32,
    n: i32,
    m: i32,
    lda: i32,
    ldb: i32,
    block_size: i32,
) {
    //#pragma omp parallel for collapse(2)
    let mut i: i32 = 0 as i32;
    while i < n {
        let mut j: i32 = 0 as i32;
        while j < m {
            let max_i2: i32 = if i + block_size < n {
                (i) + block_size
            } else {
                n
            };
            let max_j2: i32 = if j + block_size < m {
                (j) + block_size
            } else {
                m
            };
            let mut i2: i32 = i;
            while i2 < max_i2 {
                let mut j2: i32 = j;
                while j2 < max_j2 {
                    transpose4x4_SSE(
                        &mut *A.offset((i2 * lda + j2) as isize),
                        &mut *B.offset((j2 * ldb + i2) as isize),
                        lda,
                        ldb,
                    );
                    j2 += 4 as i32
                }
                i2 += 4 as i32
            }
            j += block_size
        }
        i += block_size
    }
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_bgra_transpose(
    c: *mut flow_c,
    from: *mut flow_bitmap_bgra,
    to: *mut flow_bitmap_bgra,
) -> bool {
    if (*from).w != (*to).h || (*from).h != (*to).w || (*from).fmt as u32 != (*to).fmt as u32 {
        flow_context_set_error_get_message_buffer(
            c,
            flow_status_code::Invalid_argument,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1252 as i32,
            (*::std::mem::transmute::<&[u8; 27], &[libc::c_char; 27]>(
                b"flow_bitmap_bgra_transpose\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    if (*from).fmt as u32 != flow_bgra32 as i32 as u32
        && (*from).fmt as u32 != flow_bgr32 as i32 as u32
    {
        if !flow_bitmap_bgra_transpose_slow(c, from, to) {
            flow_context_add_to_callstack(
                c,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1258 as i32,
                (*::std::mem::transmute::<&[u8; 27], &[libc::c_char; 27]>(
                    b"flow_bitmap_bgra_transpose\x00",
                ))
                    .as_ptr(),
            );
            return false;
        }
        return true;
    }
    // We require 8 when we only need 4 - in case we ever want to enable avx (like if we make it faster)
    let min_block_size: i32 = 8 as i32;
    // Strides must be multiple of required alignments
    if (*from).stride.wrapping_rem(min_block_size as u32) != 0 as i32 as u32
        || (*to).stride.wrapping_rem(min_block_size as u32) != 0 as i32 as u32
    {
        flow_context_set_error_get_message_buffer(
            c,
            flow_status_code::Invalid_argument,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1269 as i32,
            (*::std::mem::transmute::<&[u8; 27], &[libc::c_char; 27]>(
                b"flow_bitmap_bgra_transpose\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    // 256 (1024x1024 bytes) at 18.18ms, 128 at 18.6ms,  64 at 20.4ms, 16 at 25.71ms
    let block_size: i32 = 128 as i32;
    let cropped_h: i32 = (*from)
        .h
        .wrapping_sub((*from).h.wrapping_rem(min_block_size as u32))
        as i32;
    let cropped_w: i32 = (*from)
        .w
        .wrapping_sub((*from).w.wrapping_rem(min_block_size as u32))
        as i32;
    transpose_block_SSE4x4(
        (*from).pixels as *mut f32,
        (*to).pixels as *mut f32,
        cropped_h,
        cropped_w,
        (*from).stride.wrapping_div(4u32) as i32,
        (*to).stride.wrapping_div(4u32) as i32,
        block_size,
    );
    // Copy missing bits
    let mut x: u32 = cropped_h as u32;
    while x < (*to).w {
        let mut y: u32 = 0 as i32 as u32;
        while y < (*to).h {
            *(&mut *(*to).pixels.offset(
                x.wrapping_mul(4u32)
                    .wrapping_add(y.wrapping_mul((*to).stride)) as isize,
            ) as *mut libc::c_uchar as *mut u32) = *(&mut *(*from).pixels.offset(
                x.wrapping_mul((*from).stride)
                    .wrapping_add(y.wrapping_mul(4u32)) as isize,
            ) as *mut libc::c_uchar
                as *mut u32);
            y = y.wrapping_add(1)
        }
        x = x.wrapping_add(1)
    }
    let mut x_0: u32 = 0 as i32 as u32;
    while x_0 < cropped_h as u32 {
        let mut y_0: u32 = cropped_w as u32;
        while y_0 < (*to).h {
            *(&mut *(*to).pixels.offset(
                x_0.wrapping_mul(4u32)
                    .wrapping_add(y_0.wrapping_mul((*to).stride)) as isize,
            ) as *mut libc::c_uchar as *mut u32) = *(&mut *(*from).pixels.offset(
                x_0.wrapping_mul((*from).stride)
                    .wrapping_add(y_0.wrapping_mul(4u32)) as isize,
            ) as *mut libc::c_uchar
                as *mut u32);
            y_0 = y_0.wrapping_add(1)
        }
        x_0 = x_0.wrapping_add(1)
    }
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_bgra_transpose_slow(
    c: *mut flow_c,
    from: *mut flow_bitmap_bgra,
    to: *mut flow_bitmap_bgra,
) -> bool {
    if (*from).w != (*to).h || (*from).h != (*to).w || (*from).fmt as u32 != (*to).fmt as u32 {
        flow_context_set_error_get_message_buffer(
            c,
            flow_status_code::Invalid_argument,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1300 as i32,
            (*::std::mem::transmute::<&[u8; 32], &[libc::c_char; 32]>(
                b"flow_bitmap_bgra_transpose_slow\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    if (*from).fmt as u32 == flow_bgra32 as i32 as u32
        || (*from).fmt as u32 == flow_bgr32 as i32 as u32
    {
        let mut x: u32 = 0 as i32 as u32;
        while x < (*to).w {
            let mut y: u32 = 0 as i32 as u32;
            while y < (*to).h {
                *(&mut *(*to).pixels.offset(
                    x.wrapping_mul(4u32)
                        .wrapping_add(y.wrapping_mul((*to).stride)) as isize,
                ) as *mut libc::c_uchar as *mut u32) = *(&mut *(*from).pixels.offset(
                    x.wrapping_mul((*from).stride)
                        .wrapping_add(y.wrapping_mul(4u32)) as isize,
                ) as *mut libc::c_uchar
                    as *mut u32);
                y = y.wrapping_add(1)
            }
            x = x.wrapping_add(1)
        }
        return true;
    } else if (*from).fmt as u32 == flow_bgr24 as i32 as u32 {
        let from_stride: i32 = (*from).stride as i32;
        let to_stride: i32 = (*to).stride as i32;
        let mut x_0: u32 = 0 as i32 as u32;
        let mut x_stride: u32 = 0 as i32 as u32;
        let mut x_3: u32 = 0 as i32 as u32;
        while x_0 < (*to).w {
            let mut y_0: u32 = 0 as i32 as u32;
            let mut y_stride: u32 = 0 as i32 as u32;
            let mut y_3: u32 = 0 as i32 as u32;
            while y_0 < (*to).h {
                *(*to).pixels.offset(x_3.wrapping_add(y_stride) as isize) =
                    *(*from).pixels.offset(x_stride.wrapping_add(y_3) as isize);
                *(*to)
                    .pixels
                    .offset(x_3.wrapping_add(y_stride).wrapping_add(1u32) as isize) = *(*from)
                    .pixels
                    .offset(x_stride.wrapping_add(y_3).wrapping_add(1u32) as isize);
                *(*to)
                    .pixels
                    .offset(x_3.wrapping_add(y_stride).wrapping_add(2u32) as isize) = *(*from)
                    .pixels
                    .offset(x_stride.wrapping_add(y_3).wrapping_add(2u32) as isize);
                y_0 = y_0.wrapping_add(1);
                y_stride = (y_stride as u32).wrapping_add(to_stride as u32) as u32 as u32;
                y_3 = (y_3 as u32).wrapping_add(3u32) as u32 as u32
            }
            x_0 = x_0.wrapping_add(1);
            x_stride = (x_stride as u32).wrapping_add(from_stride as u32) as u32 as u32;
            x_3 = (x_3 as u32).wrapping_add(3u32) as u32 as u32
        }
        return true;
    } else {
        flow_context_set_error_get_message_buffer(
            c,
            flow_status_code::Invalid_argument,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1325 as i32,
            (*::std::mem::transmute::<&[u8; 32], &[libc::c_char; 32]>(
                b"flow_bitmap_bgra_transpose_slow\x00",
            ))
                .as_ptr(),
        );
        return false;
    };
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_convert_srgb_to_linear(
    context: *mut flow_c,
    colorcontext: *mut flow_colorcontext_info,
    src: *mut flow_bitmap_bgra,
    from_row: u32,
    dest: *mut flow_bitmap_float,
    dest_row: u32,
    row_count: u32,
) -> bool {
    if ((*src).w != (*dest).w) as i32 as libc::c_long != 0 {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1339 as i32,
            (*::std::mem::transmute::<&[u8; 41], &[libc::c_char; 41]>(
                b"flow_bitmap_float_convert_srgb_to_linear\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    if !(from_row.wrapping_add(row_count) <= (*src).h
        && dest_row.wrapping_add(row_count) <= (*dest).h) as i32 as libc::c_long
        != 0
    {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1345 as i32,
            (*::std::mem::transmute::<&[u8; 41], &[libc::c_char; 41]>(
                b"flow_bitmap_float_convert_srgb_to_linear\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    let w = (*src).w;
    let units: u32 = w * flow_pixel_format_bytes_per_pixel((*src).fmt);
    let from_step: u32 = flow_pixel_format_bytes_per_pixel((*src).fmt);
    let from_copy: u32 = flow_pixel_format_channels(flow_effective_pixel_format(src));
    let to_step: u32 = (*dest).channels;
    let copy_step: u32 = from_copy.min(to_step);
    if copy_step != 3 && copy_step != 4 {
        flow_snprintf(
            flow_context_set_error_get_message_buffer(
                context,
                flow_status_code::Unsupported_pixel_format,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1361 as i32,
                (*::std::mem::transmute::<&[u8; 41], &[libc::c_char; 41]>(
                    b"flow_bitmap_float_convert_srgb_to_linear\x00",
                ))
                    .as_ptr(),
            ),
            FLOW_ERROR_MESSAGE_SIZE as usize,
            b"copy_step=%d\x00" as *const u8 as *const libc::c_char,
            copy_step,
        );
        return false;
    }
    if copy_step == 4 && from_step != 4 && to_step != 4 {
        flow_snprintf(
            flow_context_set_error_get_message_buffer(
                context,
                flow_status_code::Unsupported_pixel_format,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1368 as i32,
                (*::std::mem::transmute::<&[u8; 41], &[libc::c_char; 41]>(
                    b"flow_bitmap_float_convert_srgb_to_linear\x00",
                ))
                    .as_ptr(),
            ),
            FLOW_ERROR_MESSAGE_SIZE as usize,
            b"copy_step=%d, from_step=%d, to_step=%d\x00" as *const u8 as *const libc::c_char,
            copy_step,
            from_step,
            to_step,
        );
        return false;
    }
    if copy_step == 4 {
        let mut row: u32 = 0 as i32 as u32;
        while row < row_count {
            let src_start: *mut u8 = (*src)
                .pixels
                .offset(from_row.wrapping_add(row).wrapping_mul((*src).stride) as isize);
            let buf: *mut f32 = (*dest).pixels.offset(
                (*dest)
                    .float_stride
                    .wrapping_mul(row.wrapping_add(dest_row)) as isize,
            );
            let mut to_x: u32 = 0 as i32 as u32;
            let mut bix: u32 = 0 as i32 as u32;
            while bix < units {
                let alpha: f32 =
                    *src_start.offset(bix.wrapping_add(3u32) as isize) as f32 / 255.0f32;
                *buf.offset(to_x as isize) = alpha
                    * flow_colorcontext_srgb_to_floatspace(
                    colorcontext,
                    *src_start.offset(bix as isize),
                );
                *buf.offset(to_x.wrapping_add(1u32) as isize) = alpha
                    * flow_colorcontext_srgb_to_floatspace(
                    colorcontext,
                    *src_start.offset(bix.wrapping_add(1u32) as isize),
                );
                *buf.offset(to_x.wrapping_add(2u32) as isize) = alpha
                    * flow_colorcontext_srgb_to_floatspace(
                    colorcontext,
                    *src_start.offset(bix.wrapping_add(2u32) as isize),
                );
                *buf.offset(to_x.wrapping_add(3u32) as isize) = alpha;
                to_x = (to_x as u32).wrapping_add(4u32) as u32 as u32;
                bix = (bix as u32).wrapping_add(4u32) as u32 as u32
            }
            row = row.wrapping_add(1)
        }
    } else if from_step == 3 && to_step == 3 {
        let mut row: u32 = 0 as i32 as u32;
        while row < row_count {
            let src_start_0: *mut u8 = (*src)
                .pixels
                .offset(from_row.wrapping_add(row).wrapping_mul((*src).stride) as isize);
            let buf: *mut f32 = (*dest).pixels.offset(
                (*dest)
                    .float_stride
                    .wrapping_mul(row.wrapping_add(dest_row)) as isize,
            );
            let mut to_x: u32 = 0 as i32 as u32;
            let mut bix: u32 = 0 as i32 as u32;
            while bix < units {
                *buf.offset(to_x as isize) = flow_colorcontext_srgb_to_floatspace(
                    colorcontext,
                    *src_start_0.offset(bix as isize),
                );
                *buf.offset(to_x.wrapping_add(1u32) as isize) =
                    flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start_0.offset(bix.wrapping_add(1u32) as isize),
                    );
                *buf.offset(to_x.wrapping_add(2u32) as isize) =
                    flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start_0.offset(bix.wrapping_add(2u32) as isize),
                    );
                to_x = (to_x as u32).wrapping_add(3u32) as u32 as u32;
                bix = (bix as u32).wrapping_add(3u32) as u32 as u32
            }
            row += 1
        }
    } else if from_step == 4 && to_step == 3 {
        let mut row: u32 = 0 as i32 as u32;
        while row < row_count {
            let src_start: *mut u8 = (*src)
                .pixels
                .offset(from_row.wrapping_add(row).wrapping_mul((*src).stride) as isize);
            let buf: *mut f32 = (*dest).pixels.offset(
                (*dest)
                    .float_stride
                    .wrapping_mul(row.wrapping_add(dest_row)) as isize,
            );
            let mut to_x: u32 = 0 as i32 as u32;
            let mut bix: u32 = 0 as i32 as u32;
            while bix < units {
                *buf.offset(to_x as isize) = flow_colorcontext_srgb_to_floatspace(
                    colorcontext,
                    *src_start.offset(bix as isize),
                );
                *buf.offset(to_x.wrapping_add(1u32) as isize) =
                    flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start.offset(bix.wrapping_add(1u32) as isize),
                    );
                *buf.offset(to_x.wrapping_add(2u32) as isize) =
                    flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start.offset(bix.wrapping_add(2u32) as isize),
                    );
                to_x = (to_x as u32).wrapping_add(3u32) as u32 as u32;
                bix = (bix as u32).wrapping_add(4u32) as u32 as u32
            }
            row += 1
        }
    } else if from_step == 3 && to_step == 4 {
        let mut row: u32 = 0 as i32 as u32;
        while row < row_count {
            let src_start: *mut u8 = (*src)
                .pixels
                .offset(from_row.wrapping_add(row).wrapping_mul((*src).stride) as isize);
            let buf: *mut f32 = (*dest).pixels.offset(
                (*dest)
                    .float_stride
                    .wrapping_mul(row.wrapping_add(dest_row)) as isize,
            );
            let mut to_x: u32 = 0 as i32 as u32;
            let mut bix: u32 = 0 as i32 as u32;
            while bix < units {
                *buf.offset(to_x as isize) = flow_colorcontext_srgb_to_floatspace(
                    colorcontext,
                    *src_start.offset(bix as isize),
                );
                *buf.offset(to_x.wrapping_add(1u32) as isize) =
                    flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start.offset(bix.wrapping_add(1u32) as isize),
                    );
                *buf.offset(to_x.wrapping_add(2u32) as isize) =
                    flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start.offset(bix.wrapping_add(2u32) as isize),
                    );
                to_x = (to_x as u32).wrapping_add(4u32) as u32 as u32;
                bix = (bix as u32).wrapping_add(3u32) as u32 as u32
            }
            row += 1
        }
    } else if from_step == 4 && to_step == 4 {
        let mut row: u32 = 0 as i32 as u32;
        while row < row_count {
            let src_start: *mut u8 = (*src)
                .pixels
                .offset(from_row.wrapping_add(row).wrapping_mul((*src).stride) as isize);
            let buf: *mut f32 = (*dest).pixels.offset(
                (*dest)
                    .float_stride
                    .wrapping_mul(row.wrapping_add(dest_row)) as isize,
            );
            let mut to_x: u32 = 0 as i32 as u32;
            let mut bix: u32 = 0 as i32 as u32;
            while bix < units {
                *buf.offset(to_x as isize) = flow_colorcontext_srgb_to_floatspace(
                    colorcontext,
                    *src_start.offset(bix as isize),
                );
                *buf.offset(to_x.wrapping_add(1u32) as isize) =
                    flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start.offset(bix.wrapping_add(1u32) as isize),
                    );
                *buf.offset(to_x.wrapping_add(2u32) as isize) =
                    flow_colorcontext_srgb_to_floatspace(
                        colorcontext,
                        *src_start.offset(bix.wrapping_add(2u32) as isize),
                    );
                to_x = (to_x as u32).wrapping_add(4u32) as u32 as u32;
                bix = (bix as u32).wrapping_add(4u32) as u32 as u32
            }
            row += 1
        }
    } else {
        flow_snprintf(
            flow_context_set_error_get_message_buffer(
                context,
                flow_status_code::Unsupported_pixel_format,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1411 as i32,
                (*::std::mem::transmute::<&[u8; 41], &[libc::c_char; 41]>(
                    b"flow_bitmap_float_convert_srgb_to_linear\x00",
                ))
                    .as_ptr(),
            ),
            FLOW_ERROR_MESSAGE_SIZE as usize,
            b"copy_step=%d, from_step=%d, to_step=%d\x00" as *const u8 as *const libc::c_char,
            copy_step,
            from_step,
            to_step,
        );
        return false;
    }
    return true;
}
/*
static void unpack24bitRow(u32 width, unsigned char* sourceLine, unsigned char* destArray){
    for (u32 i = 0; i < width; i++){

        memcpy(destArray + i * 4, sourceLine + i * 3, 3);
        destArray[i * 4 + 3] = 255;
    }
}
*/
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_bgra_flip_vertical(
    context: *mut flow_c,
    b: *mut flow_bitmap_bgra,
) -> bool {
    let swap: *mut libc::c_void = flow_context_malloc(
        context,
        (*b).stride as usize,
        ::std::mem::transmute::<libc::intptr_t, flow_destructor_function>(NULL as libc::intptr_t),
        context as *mut libc::c_void,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        1430 as i32,
    );
    if swap.is_null() {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Out_of_memory,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1432 as i32,
            (*::std::mem::transmute::<&[u8; 31], &[libc::c_char; 31]>(
                b"flow_bitmap_bgra_flip_vertical\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    // Dont' copy the full stride (padding), it could be windowed!
    // Todo: try multiple swap rows? 5ms isn't bad, but could be better
    let row_length: u32 = (*b).stride.min(
        (*b).w
            .wrapping_mul(flow_pixel_format_bytes_per_pixel((*b).fmt)),
    );
    let mut i: u32 = 0 as i32 as u32;
    while i < (*b).h.wrapping_div(2u32) {
        let top: *mut libc::c_void =
            (*b).pixels.offset(i.wrapping_mul((*b).stride) as isize) as *mut libc::c_void;
        let bottom: *mut libc::c_void = (*b).pixels.offset(
            (*b).h
                .wrapping_sub(1u32)
                .wrapping_sub(i)
                .wrapping_mul((*b).stride) as isize,
        ) as *mut libc::c_void;
        memcpy(swap, top, row_length as u64);
        memcpy(top, bottom, row_length as u64);
        memcpy(bottom, swap, row_length as u64);
        i = i.wrapping_add(1)
    }
    flow_deprecated_free(
        context,
        swap,
        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
        1445 as i32,
    );
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_bgra_flip_horizontal(
    _context: *mut flow_c,
    b: *mut flow_bitmap_bgra,
) -> bool {
    if (*b).fmt as u32 == flow_bgra32 as i32 as u32 || (*b).fmt as u32 == flow_bgr32 as i32 as u32 {
        // 12ms simple
        let mut y: u32 = 0 as i32 as u32;
        while y < (*b).h {
            let mut left: *mut u32 =
                (*b).pixels.offset(y.wrapping_mul((*b).stride) as isize) as *mut u32;
            let mut right: *mut u32 = (*b)
                .pixels
                .offset(y.wrapping_mul((*b).stride) as isize)
                .offset((4u32).wrapping_mul((*b).w.wrapping_sub(1u32)) as isize)
                as *mut u32;
            while left < right {
                let swap: u32 = *left;
                *left = *right;
                *right = swap;
                left = left.offset(1);
                right = right.offset(-1)
            }
            y = y.wrapping_add(1)
        }
    } else if (*b).fmt as u32 == flow_bgr24 as i32 as u32 {
        let mut swap_0: [u32; 4] = [0; 4];
        // Dont' copy the full stride (padding), it could be windowed!
        let mut y_0: u32 = 0 as i32 as u32;
        while y_0 < (*b).h {
            let mut left_0: *mut u8 = (*b).pixels.offset(y_0.wrapping_mul((*b).stride) as isize);
            let mut right_0: *mut u8 = (*b)
                .pixels
                .offset(y_0.wrapping_mul((*b).stride) as isize)
                .offset((3u32).wrapping_mul((*b).w.wrapping_sub(1u32)) as isize);
            while left_0 < right_0 {
                memcpy(
                    &mut swap_0 as *mut [u32; 4] as *mut libc::c_void,
                    left_0 as *const libc::c_void,
                    3 as i32 as u64,
                );
                memcpy(
                    left_0 as *mut libc::c_void,
                    right_0 as *const libc::c_void,
                    3 as i32 as u64,
                );
                memcpy(
                    right_0 as *mut libc::c_void,
                    &mut swap_0 as *mut [u32; 4] as *const libc::c_void,
                    3 as i32 as u64,
                );
                left_0 = left_0.offset(3 as i32 as isize);
                right_0 = right_0.offset(-(3 as i32 as isize))
            }
            y_0 = y_0.wrapping_add(1)
        }
    } else {
        let mut swap_1: [u32; 4] = [0; 4];
        // Dont' copy the full stride (padding), it could be windowed!
        let mut y_1: u32 = 0 as i32 as u32;
        while y_1 < (*b).h {
            let mut left_1: *mut u8 = (*b).pixels.offset(y_1.wrapping_mul((*b).stride) as isize);
            let mut right_1: *mut u8 = (*b)
                .pixels
                .offset(y_1.wrapping_mul((*b).stride) as isize)
                .offset(
                    flow_pixel_format_bytes_per_pixel((*b).fmt)
                        .wrapping_mul((*b).w.wrapping_sub(1u32)) as isize,
                );
            while left_1 < right_1 {
                memcpy(
                    &mut swap_1 as *mut [u32; 4] as *mut libc::c_void,
                    left_1 as *const libc::c_void,
                    flow_pixel_format_bytes_per_pixel((*b).fmt) as u64,
                );
                memcpy(
                    left_1 as *mut libc::c_void,
                    right_1 as *const libc::c_void,
                    flow_pixel_format_bytes_per_pixel((*b).fmt) as u64,
                );
                memcpy(
                    right_1 as *mut libc::c_void,
                    &mut swap_1 as *mut [u32; 4] as *const libc::c_void,
                    flow_pixel_format_bytes_per_pixel((*b).fmt) as u64,
                );
                left_1 = left_1.offset(flow_pixel_format_bytes_per_pixel((*b).fmt) as isize);
                right_1 = right_1.offset(-(flow_pixel_format_bytes_per_pixel((*b).fmt) as isize))
            }
            y_1 = y_1.wrapping_add(1)
        }
    }
    return true;
}
unsafe extern "C" fn flow_bitmap_float_blend_matte(
    _context: *mut flow_c,
    colorcontext: *mut flow_colorcontext_info,
    src: *mut flow_bitmap_float,
    from_row: u32,
    row_count: u32,
    matte: *const u8,
) -> bool {
    // We assume that matte is BGRA, regardless.
    let matte_a: f32 = *matte.offset(3 as i32 as isize) as f32 / 255.0f32;
    let b: f32 = flow_colorcontext_srgb_to_floatspace(colorcontext, *matte.offset(0));
    let g: f32 = flow_colorcontext_srgb_to_floatspace(colorcontext, *matte.offset(1));
    let r: f32 = flow_colorcontext_srgb_to_floatspace(colorcontext, *matte.offset(2));
    let mut row: u32 = from_row;
    while row < from_row.wrapping_add(row_count) {
        let start_ix: u32 = row.wrapping_mul((*src).float_stride);
        let end_ix: u32 = start_ix.wrapping_add((*src).w.wrapping_mul((*src).channels));
        let mut ix: u32 = start_ix;
        while ix < end_ix {
            let src_a: f32 = *(*src).pixels.offset(ix.wrapping_add(3u32) as isize);
            let a: f32 = (1.0f32 - src_a) * matte_a;
            let final_alpha: f32 = src_a + a;
            *(*src).pixels.offset(ix as isize) =
                (*(*src).pixels.offset(ix as isize) + b * a) / final_alpha;
            *(*src).pixels.offset(ix.wrapping_add(1u32) as isize) =
                (*(*src).pixels.offset(ix.wrapping_add(1u32) as isize) + g * a) / final_alpha;
            *(*src).pixels.offset(ix.wrapping_add(2u32) as isize) =
                (*(*src).pixels.offset(ix.wrapping_add(2u32) as isize) + r * a) / final_alpha;
            *(*src).pixels.offset(ix.wrapping_add(3u32) as isize) = final_alpha;
            ix = (ix as u32).wrapping_add(4u32) as u32 as u32
        }
        row = row.wrapping_add(1)
    }
    // Ensure alpha is demultiplied
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_demultiply_alpha(
    _context: *mut flow_c,
    src: *mut flow_bitmap_float,
    from_row: u32,
    row_count: u32,
) -> bool {
    let mut row: u32 = from_row;
    while row < from_row.wrapping_add(row_count) {
        let start_ix: u32 = row.wrapping_mul((*src).float_stride);
        let end_ix: u32 = start_ix.wrapping_add((*src).w.wrapping_mul((*src).channels));
        let mut ix: u32 = start_ix;
        while ix < end_ix {
            let alpha: f32 = *(*src).pixels.offset(ix.wrapping_add(3u32) as isize);
            if alpha > 0 as i32 as f32 {
                *(*src).pixels.offset(ix as isize) /= alpha;
                *(*src).pixels.offset(ix.wrapping_add(1u32) as isize) /= alpha;
                *(*src).pixels.offset(ix.wrapping_add(2u32) as isize) /= alpha
            }
            ix = (ix as u32).wrapping_add(4u32) as u32 as u32
        }
        row = row.wrapping_add(1)
    }
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_copy_linear_over_srgb(
    _context: *mut flow_c,
    colorcontext: *mut flow_colorcontext_info,
    src: *mut flow_bitmap_float,
    from_row: u32,
    dest: *mut flow_bitmap_bgra,
    dest_row: u32,
    row_count: u32,
    from_col: u32,
    col_count: u32,
    transpose: bool,
) -> bool {
    let dest_bytes_pp: u32 = flow_pixel_format_bytes_per_pixel((*dest).fmt);
    let srcitems: u32 = from_col
        .wrapping_add(col_count)
        .min((*src).w)
        .wrapping_mul((*src).channels);
    let dest_fmt: flow_pixel_format = flow_effective_pixel_format(dest);
    let ch: u32 = (*src).channels;
    let copy_alpha: bool = dest_fmt as u32 == flow_bgra32 as i32 as u32
        && ch == 4 as i32 as u32
        && (*src).alpha_meaningful as i32 != 0;
    let clean_alpha: bool = !copy_alpha && dest_fmt as u32 == flow_bgra32 as i32 as u32;
    let dest_row_stride: u32 = if transpose as i32 != 0 {
        dest_bytes_pp
    } else {
        (*dest).stride
    };
    let dest_pixel_stride: u32 = if transpose as i32 != 0 {
        (*dest).stride
    } else {
        dest_bytes_pp
    };
    if dest_pixel_stride == 4 as i32 as u32 {
        if ch == 3 as i32 as u32 {
            if copy_alpha && !clean_alpha {
                let mut row: u32 = 0 as i32 as u32;
                while row < row_count {
                    let src_row: *mut f32 =
                        (*src)
                            .pixels
                            .offset(row.wrapping_add(from_row).wrapping_mul((*src).float_stride)
                                as isize);
                    let mut dest_row_bytes: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix: u32 = from_col.wrapping_mul(3u32);
                    while ix < srcitems {
                        *dest_row_bytes.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row.offset(ix as isize),
                        );
                        *dest_row_bytes.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row.offset(ix.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row.offset(ix.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes.offset(3 as i32 as isize) = uchar_clamp_ff(
                            *src_row.offset(ix.wrapping_add(3u32) as isize) * 255.0f32,
                        );
                        dest_row_bytes = dest_row_bytes.offset(4 as i32 as isize);
                        ix = (ix as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row = row.wrapping_add(1)
                }
            }
            if !copy_alpha && !clean_alpha {
                let mut row_0: u32 = 0 as i32 as u32;
                while row_0 < row_count {
                    let src_row_0: *mut f32 = (*src).pixels.offset(
                        row_0
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_0: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_0).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix_0: u32 = from_col.wrapping_mul(3u32);
                    while ix_0 < srcitems {
                        *dest_row_bytes_0.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_0.offset(ix_0 as isize),
                        );
                        *dest_row_bytes_0.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_0.offset(ix_0.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_0.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_0.offset(ix_0.wrapping_add(2u32) as isize),
                        );
                        dest_row_bytes_0 = dest_row_bytes_0.offset(4 as i32 as isize);
                        ix_0 = (ix_0 as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row_0 = row_0.wrapping_add(1)
                }
            }
            if !copy_alpha && clean_alpha {
                let mut row_1: u32 = 0 as i32 as u32;
                while row_1 < row_count {
                    let src_row_1: *mut f32 = (*src).pixels.offset(
                        row_1
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_1: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_1).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix_1: u32 = from_col.wrapping_mul(3u32);
                    while ix_1 < srcitems {
                        *dest_row_bytes_1.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_1.offset(ix_1 as isize),
                        );
                        *dest_row_bytes_1.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_1.offset(ix_1.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_1.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_1.offset(ix_1.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_1.offset(3 as i32 as isize) = 0xff as i32 as u8;
                        dest_row_bytes_1 = dest_row_bytes_1.offset(4 as i32 as isize);
                        ix_1 = (ix_1 as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row_1 = row_1.wrapping_add(1)
                }
            }
        }
        if ch == 4 as i32 as u32 {
            if copy_alpha && !clean_alpha {
                let mut row_2: u32 = 0 as i32 as u32;
                while row_2 < row_count {
                    let src_row_2: *mut f32 = (*src).pixels.offset(
                        row_2
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_2: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_2).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix_2: u32 = from_col.wrapping_mul(4u32);
                    while ix_2 < srcitems {
                        *dest_row_bytes_2.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_2.offset(ix_2 as isize),
                        );
                        *dest_row_bytes_2.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_2.offset(ix_2.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_2.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_2.offset(ix_2.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_2.offset(3 as i32 as isize) = uchar_clamp_ff(
                            *src_row_2.offset(ix_2.wrapping_add(3u32) as isize) * 255.0f32,
                        );
                        dest_row_bytes_2 = dest_row_bytes_2.offset(4 as i32 as isize);
                        ix_2 = (ix_2 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_2 = row_2.wrapping_add(1)
                }
            }
            if !copy_alpha && !clean_alpha {
                let mut row_3: u32 = 0 as i32 as u32;
                while row_3 < row_count {
                    let src_row_3: *mut f32 = (*src).pixels.offset(
                        row_3
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_3: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_3).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix_3: u32 = from_col.wrapping_mul(4u32);
                    while ix_3 < srcitems {
                        *dest_row_bytes_3.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_3.offset(ix_3 as isize),
                        );
                        *dest_row_bytes_3.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_3.offset(ix_3.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_3.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_3.offset(ix_3.wrapping_add(2u32) as isize),
                        );
                        dest_row_bytes_3 = dest_row_bytes_3.offset(4 as i32 as isize);
                        ix_3 = (ix_3 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_3 = row_3.wrapping_add(1)
                }
            }
            if !copy_alpha && clean_alpha {
                let mut row_4: u32 = 0 as i32 as u32;
                while row_4 < row_count {
                    let src_row_4: *mut f32 = (*src).pixels.offset(
                        row_4
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_4: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_4).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(4u32) as isize);
                    let mut ix_4: u32 = from_col.wrapping_mul(4u32);
                    while ix_4 < srcitems {
                        *dest_row_bytes_4.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_4.offset(ix_4 as isize),
                        );
                        *dest_row_bytes_4.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_4.offset(ix_4.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_4.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_4.offset(ix_4.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_4.offset(3 as i32 as isize) = 0xff as i32 as u8;
                        dest_row_bytes_4 = dest_row_bytes_4.offset(4 as i32 as isize);
                        ix_4 = (ix_4 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_4 = row_4.wrapping_add(1)
                }
            }
        }
    } else {
        if ch == 3 as i32 as u32 {
            if copy_alpha && !clean_alpha {
                let mut row_5: u32 = 0 as i32 as u32;
                while row_5 < row_count {
                    let src_row_5: *mut f32 = (*src).pixels.offset(
                        row_5
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_5: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_5).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_5: u32 = from_col.wrapping_mul(3u32);
                    while ix_5 < srcitems {
                        *dest_row_bytes_5.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_5.offset(ix_5 as isize),
                        );
                        *dest_row_bytes_5.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_5.offset(ix_5.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_5.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_5.offset(ix_5.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_5.offset(3 as i32 as isize) = uchar_clamp_ff(
                            *src_row_5.offset(ix_5.wrapping_add(3u32) as isize) * 255.0f32,
                        );
                        dest_row_bytes_5 = dest_row_bytes_5.offset(dest_pixel_stride as isize);
                        ix_5 = (ix_5 as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row_5 = row_5.wrapping_add(1)
                }
            }
            if !copy_alpha && !clean_alpha {
                let mut row_6: u32 = 0 as i32 as u32;
                while row_6 < row_count {
                    let src_row_6: *mut f32 = (*src).pixels.offset(
                        row_6
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_6: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_6).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_6: u32 = from_col.wrapping_mul(3u32);
                    while ix_6 < srcitems {
                        *dest_row_bytes_6.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_6.offset(ix_6 as isize),
                        );
                        *dest_row_bytes_6.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_6.offset(ix_6.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_6.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_6.offset(ix_6.wrapping_add(2u32) as isize),
                        );
                        dest_row_bytes_6 = dest_row_bytes_6.offset(dest_pixel_stride as isize);
                        ix_6 = (ix_6 as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row_6 = row_6.wrapping_add(1)
                }
            }
            if !copy_alpha && clean_alpha {
                let mut row_7: u32 = 0 as i32 as u32;
                while row_7 < row_count {
                    let src_row_7: *mut f32 = (*src).pixels.offset(
                        row_7
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_7: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_7).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_7: u32 = from_col.wrapping_mul(3u32);
                    while ix_7 < srcitems {
                        *dest_row_bytes_7.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_7.offset(ix_7 as isize),
                        );
                        *dest_row_bytes_7.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_7.offset(ix_7.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_7.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_7.offset(ix_7.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_7.offset(3 as i32 as isize) = 0xff as i32 as u8;
                        dest_row_bytes_7 = dest_row_bytes_7.offset(dest_pixel_stride as isize);
                        ix_7 = (ix_7 as u32).wrapping_add(3u32) as u32 as u32
                    }
                    row_7 = row_7.wrapping_add(1)
                }
            }
        }
        if ch == 4 as i32 as u32 {
            if copy_alpha && !clean_alpha {
                let mut row_8: u32 = 0 as i32 as u32;
                while row_8 < row_count {
                    let src_row_8: *mut f32 = (*src).pixels.offset(
                        row_8
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_8: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_8).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_8: u32 = from_col.wrapping_mul(4u32);
                    while ix_8 < srcitems {
                        *dest_row_bytes_8.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_8.offset(ix_8 as isize),
                        );
                        *dest_row_bytes_8.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_8.offset(ix_8.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_8.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_8.offset(ix_8.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_8.offset(3 as i32 as isize) = uchar_clamp_ff(
                            *src_row_8.offset(ix_8.wrapping_add(3u32) as isize) * 255.0f32,
                        );
                        dest_row_bytes_8 = dest_row_bytes_8.offset(dest_pixel_stride as isize);
                        ix_8 = (ix_8 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_8 = row_8.wrapping_add(1)
                }
            }
            if !copy_alpha && !clean_alpha {
                let mut row_9: u32 = 0 as i32 as u32;
                while row_9 < row_count {
                    let src_row_9: *mut f32 = (*src).pixels.offset(
                        row_9
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_9: *mut u8 = (*dest)
                        .pixels
                        .offset(dest_row.wrapping_add(row_9).wrapping_mul(dest_row_stride) as isize)
                        .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_9: u32 = from_col.wrapping_mul(4u32);
                    while ix_9 < srcitems {
                        *dest_row_bytes_9.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_9.offset(ix_9 as isize),
                        );
                        *dest_row_bytes_9.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_9.offset(ix_9.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_9.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_9.offset(ix_9.wrapping_add(2u32) as isize),
                        );
                        dest_row_bytes_9 = dest_row_bytes_9.offset(dest_pixel_stride as isize);
                        ix_9 = (ix_9 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_9 = row_9.wrapping_add(1)
                }
            }
            if !copy_alpha && clean_alpha {
                let mut row_10: u32 = 0 as i32 as u32;
                while row_10 < row_count {
                    let src_row_10: *mut f32 = (*src).pixels.offset(
                        row_10
                            .wrapping_add(from_row)
                            .wrapping_mul((*src).float_stride) as isize,
                    );
                    let mut dest_row_bytes_10: *mut u8 =
                        (*dest)
                            .pixels
                            .offset(dest_row.wrapping_add(row_10).wrapping_mul(dest_row_stride)
                                as isize)
                            .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
                    let mut ix_10: u32 = from_col.wrapping_mul(4u32);
                    while ix_10 < srcitems {
                        *dest_row_bytes_10.offset(0) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_10.offset(ix_10 as isize),
                        );
                        *dest_row_bytes_10.offset(1) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_10.offset(ix_10.wrapping_add(1u32) as isize),
                        );
                        *dest_row_bytes_10.offset(2) = flow_colorcontext_floatspace_to_srgb(
                            colorcontext,
                            *src_row_10.offset(ix_10.wrapping_add(2u32) as isize),
                        );
                        *dest_row_bytes_10.offset(3 as i32 as isize) = 0xff as i32 as u8;
                        dest_row_bytes_10 = dest_row_bytes_10.offset(dest_pixel_stride as isize);
                        ix_10 = (ix_10 as u32).wrapping_add(4u32) as u32 as u32
                    }
                    row_10 = row_10.wrapping_add(1)
                }
            }
        }
    }
    return true;
}
unsafe extern "C" fn BitmapFloat_compose_linear_over_srgb(
    _context: *mut flow_c,
    colorcontext: *mut flow_colorcontext_info,
    src: *mut flow_bitmap_float,
    from_row: u32,
    dest: *mut flow_bitmap_bgra,
    dest_row: u32,
    row_count: u32,
    from_col: u32,
    col_count: u32,
    transpose: bool,
) -> bool {
    let dest_bytes_pp: u32 = flow_pixel_format_bytes_per_pixel((*dest).fmt);
    let dest_row_stride: u32 = if transpose as i32 != 0 {
        dest_bytes_pp
    } else {
        (*dest).stride
    };
    let dest_pixel_stride: u32 = if transpose as i32 != 0 {
        (*dest).stride
    } else {
        dest_bytes_pp
    };
    let srcitems: u32 = from_col
        .wrapping_add(col_count)
        .min((*src).w)
        .wrapping_mul((*src).channels);
    let ch: u32 = (*src).channels;
    let dest_effective_format: flow_pixel_format = flow_effective_pixel_format(dest);
    let dest_alpha: bool = dest_effective_format as u32 == flow_bgra32 as i32 as u32;
    let dest_alpha_index: u8 = if dest_alpha as i32 != 0 {
        3 as i32
    } else {
        0 as i32
    } as u8;
    let dest_alpha_to_float_coeff: f32 = if dest_alpha as i32 != 0 {
        (1.0f32) / 255.0f32
    } else {
        0.0f32
    };
    let dest_alpha_to_float_offset: f32 = if dest_alpha as i32 != 0 {
        0.0f32
    } else {
        1.0f32
    };
    let mut row: u32 = 0 as i32 as u32;
    while row < row_count {
        // const float * const __restrict src_row = src->pixels + (row + from_row) * src->float_stride;
        let src_row: *mut f32 = (*src)
            .pixels
            .offset(row.wrapping_add(from_row).wrapping_mul((*src).float_stride) as isize);
        let mut dest_row_bytes: *mut u8 = (*dest)
            .pixels
            .offset(dest_row.wrapping_add(row).wrapping_mul(dest_row_stride) as isize)
            .offset(from_col.wrapping_mul(dest_pixel_stride) as isize);
        let mut ix: u32 = from_col.wrapping_mul(ch);
        while ix < srcitems {
            let dest_b: u8 = *dest_row_bytes.offset(0);
            let dest_g: u8 = *dest_row_bytes.offset(1);
            let dest_r: u8 = *dest_row_bytes.offset(2);
            let dest_a: u8 = *dest_row_bytes.offset(dest_alpha_index as isize);
            let src_b: f32 = *src_row.offset(ix.wrapping_add(0u32) as isize);
            let src_g: f32 = *src_row.offset(ix.wrapping_add(1u32) as isize);
            let src_r: f32 = *src_row.offset(ix.wrapping_add(2u32) as isize);
            let src_a: f32 = *src_row.offset(ix.wrapping_add(3u32) as isize);
            let a: f32 = (1.0f32 - src_a)
                * (dest_alpha_to_float_coeff * dest_a as i32 as f32 + dest_alpha_to_float_offset);
            let b: f32 = flow_colorcontext_srgb_to_floatspace(colorcontext, dest_b) * a + src_b;
            let g: f32 = flow_colorcontext_srgb_to_floatspace(colorcontext, dest_g) * a + src_g;
            let r: f32 = flow_colorcontext_srgb_to_floatspace(colorcontext, dest_r) * a + src_r;
            let final_alpha: f32 = src_a + a;
            *dest_row_bytes.offset(0) =
                flow_colorcontext_floatspace_to_srgb(colorcontext, b / final_alpha);
            *dest_row_bytes.offset(1) =
                flow_colorcontext_floatspace_to_srgb(colorcontext, g / final_alpha);
            *dest_row_bytes.offset(2) =
                flow_colorcontext_floatspace_to_srgb(colorcontext, r / final_alpha);
            if dest_alpha {
                *dest_row_bytes.offset(3 as i32 as isize) =
                    uchar_clamp_ff(final_alpha * 255 as i32 as f32)
            }
            // TODO: split out 4 and 3 so compiler can vectorize maybe?
            dest_row_bytes = dest_row_bytes.offset(dest_pixel_stride as isize);
            ix = (ix as u32).wrapping_add(ch) as u32 as u32
        }
        row = row.wrapping_add(1)
    }
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_composite_linear_over_srgb(
    context: *mut flow_c,
    colorcontext: *mut flow_colorcontext_info,
    src_mut: *mut flow_bitmap_float,
    from_row: u32,
    dest: *mut flow_bitmap_bgra,
    dest_row: u32,
    row_count: u32,
    transpose: bool,
) -> bool {
    if if transpose as i32 != 0 {
        ((*src_mut).w != (*dest).h) as i32
    } else {
        ((*src_mut).w != (*dest).w) as i32
    } != 0
    {
        // TODO: Add more bounds checks
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1699 as i32,
            (*::std::mem::transmute::<&[u8; 45], &[libc::c_char; 45]>(
                b"flow_bitmap_float_composite_linear_over_srgb\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    if (*dest).compositing_mode as u32 == flow_bitmap_compositing_blend_with_self as i32 as u32
        && (*src_mut).alpha_meaningful as i32 != 0
        && (*src_mut).channels == 4 as i32 as u32
    {
        if !(*src_mut).alpha_premultiplied {
            // Something went wrong. We should always have alpha premultiplied.
            flow_context_set_error_get_message_buffer(
                context,
                flow_status_code::Invalid_internal_state,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1706 as i32,
                (*::std::mem::transmute::<&[u8; 45], &[libc::c_char; 45]>(
                    b"flow_bitmap_float_composite_linear_over_srgb\x00",
                ))
                    .as_ptr(),
            );
            return false;
        }
        // Compose
        if !BitmapFloat_compose_linear_over_srgb(
            context,
            colorcontext,
            src_mut,
            from_row,
            dest,
            dest_row,
            row_count,
            0 as i32 as u32,
            (*src_mut).w,
            transpose,
        ) {
            flow_context_add_to_callstack(
                context,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1712 as i32,
                (*::std::mem::transmute::<&[u8; 45], &[libc::c_char; 45]>(
                    b"flow_bitmap_float_composite_linear_over_srgb\x00",
                ))
                    .as_ptr(),
            );
            return false;
        }
    } else {
        if (*src_mut).channels == 4 as i32 as u32 && (*src_mut).alpha_meaningful as i32 != 0 {
            let mut demultiply: bool = (*src_mut).alpha_premultiplied;
            if (*dest).compositing_mode as u32
                == flow_bitmap_compositing_blend_with_matte as i32 as u32
            {
                if !flow_bitmap_float_blend_matte(
                    context,
                    colorcontext,
                    src_mut,
                    from_row,
                    row_count,
                    (*dest).matte_color.as_mut_ptr(),
                ) {
                    flow_context_add_to_callstack(
                        context,
                        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                        1722 as i32,
                        (*::std::mem::transmute::<&[u8; 45], &[libc::c_char; 45]>(
                            b"flow_bitmap_float_composite_linear_over_srgb\x00",
                        ))
                            .as_ptr(),
                    );
                    return false;
                }
                demultiply = false
            }
            if demultiply {
                // Demultiply before copy
                if !flow_bitmap_float_demultiply_alpha(context, src_mut, from_row, row_count) {
                    flow_context_add_to_callstack(
                        context,
                        b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                        1730 as i32,
                        (*::std::mem::transmute::<&[u8; 45], &[libc::c_char; 45]>(
                            b"flow_bitmap_float_composite_linear_over_srgb\x00",
                        ))
                            .as_ptr(),
                    );
                    return false;
                }
            }
        }
        // Copy/overwrite
        if !flow_bitmap_float_copy_linear_over_srgb(
            context,
            colorcontext,
            src_mut,
            from_row,
            dest,
            dest_row,
            row_count,
            0 as i32 as u32,
            (*src_mut).w,
            transpose,
        ) {
            flow_context_add_to_callstack(
                context,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1738 as i32,
                (*::std::mem::transmute::<&[u8; 45], &[libc::c_char; 45]>(
                    b"flow_bitmap_float_composite_linear_over_srgb\x00",
                ))
                    .as_ptr(),
            ); // Don't access rows past the end of the bitmap
            return false;
        }
    } // This algorithm can't handle padding, if present
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_linear_to_luv_rows(
    context: *mut flow_c,
    bit: *mut flow_bitmap_float,
    start_row: u32,
    row_count: u32,
) -> bool {
    if !(start_row.wrapping_add(row_count) <= (*bit).h) {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1751 as i32,
            (*::std::mem::transmute::<&[u8; 37], &[libc::c_char; 37]>(
                b"flow_bitmap_float_linear_to_luv_rows\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    if (*bit).w.wrapping_mul((*bit).channels) != (*bit).float_stride {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1755 as i32,
            (*::std::mem::transmute::<&[u8; 37], &[libc::c_char; 37]>(
                b"flow_bitmap_float_linear_to_luv_rows\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    let start_at: *mut f32 = (*bit)
        .pixels
        .offset((*bit).float_stride.wrapping_mul(start_row) as isize);
    let end_at: *const f32 = (*bit).pixels.offset(
        (*bit)
            .float_stride
            .wrapping_mul(start_row.wrapping_add(row_count)) as isize,
    );
    let mut pix: *mut f32 = start_at;
    while pix < end_at as *mut f32 {
        linear_to_luv(pix);
        pix = pix.offset(1)
    }
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_luv_to_linear_rows(
    context: *mut flow_c,
    bit: *mut flow_bitmap_float,
    start_row: u32,
    row_count: u32,
) -> bool {
    if !(start_row.wrapping_add(row_count) <= (*bit).h) {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1772 as i32,
            (*::std::mem::transmute::<&[u8; 37], &[libc::c_char; 37]>(
                b"flow_bitmap_float_luv_to_linear_rows\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    if (*bit).w.wrapping_mul((*bit).channels) != (*bit).float_stride {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Invalid_internal_state,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1776 as i32,
            (*::std::mem::transmute::<&[u8; 37], &[libc::c_char; 37]>(
                b"flow_bitmap_float_luv_to_linear_rows\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    let start_at: *mut f32 = (*bit)
        .pixels
        .offset((*bit).float_stride.wrapping_mul(start_row) as isize);
    let end_at: *const f32 = (*bit).pixels.offset(
        (*bit)
            .float_stride
            .wrapping_mul(start_row.wrapping_add(row_count)) as isize,
    );
    let mut pix: *mut f32 = start_at;
    while pix < end_at as *mut f32 {
        luv_to_linear(pix);
        pix = pix.offset(1)
    }
    return true;
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_bgra_apply_color_matrix(
    context: *mut flow_c,
    bmp: *mut flow_bitmap_bgra,
    row: u32,
    count: u32,
    m: *const *mut f32,
) -> bool {
    let stride: u32 = (*bmp).stride;
    let ch: u32 = flow_pixel_format_bytes_per_pixel((*bmp).fmt);
    let w: u32 = (*bmp).w;
    let h: u32 = row.wrapping_add(count).min((*bmp).h);
    let m40: f32 = *(*m.offset(4 as i32 as isize)).offset(0) * 255.0f32;
    let m41: f32 = *(*m.offset(4 as i32 as isize)).offset(1) * 255.0f32;
    let m42: f32 = *(*m.offset(4 as i32 as isize)).offset(2) * 255.0f32;
    let m43: f32 = *(*m.offset(4 as i32 as isize)).offset(3 as i32 as isize) * 255.0f32;
    if ch == 4 as i32 as u32 {
        let mut y: u32 = row;
        while y < h {
            let mut x: u32 = 0 as i32 as u32;
            while x < w {
                let data: *mut u8 = (*bmp)
                    .pixels
                    .offset(stride.wrapping_mul(y) as isize)
                    .offset(x.wrapping_mul(ch) as isize);
                let r: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(0) * *data.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(0) * *data.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(0) * *data.offset(0) as i32 as f32
                        + *(*m.offset(3 as i32 as isize)).offset(0)
                        * *data.offset(3 as i32 as isize) as i32 as f32
                        + m40,
                );
                let g: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(1) * *data.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(1) * *data.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(1) * *data.offset(0) as i32 as f32
                        + *(*m.offset(3 as i32 as isize)).offset(1)
                        * *data.offset(3 as i32 as isize) as i32 as f32
                        + m41,
                );
                let b: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(2) * *data.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(2) * *data.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(2) * *data.offset(0) as i32 as f32
                        + *(*m.offset(3 as i32 as isize)).offset(2)
                        * *data.offset(3 as i32 as isize) as i32 as f32
                        + m42,
                );
                let a: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(3 as i32 as isize) * *data.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(3 as i32 as isize) * *data.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(3 as i32 as isize) * *data.offset(0) as i32 as f32
                        + *(*m.offset(3 as i32 as isize)).offset(3 as i32 as isize)
                        * *data.offset(3 as i32 as isize) as i32 as f32
                        + m43,
                );
                let newdata: *mut u8 = (*bmp)
                    .pixels
                    .offset(stride.wrapping_mul(y) as isize)
                    .offset(x.wrapping_mul(ch) as isize);
                *newdata.offset(0) = b;
                *newdata.offset(1) = g;
                *newdata.offset(2) = r;
                *newdata.offset(3 as i32 as isize) = a;
                x = x.wrapping_add(1)
            }
            y = y.wrapping_add(1)
        }
    } else if ch == 3 as i32 as u32 {
        let mut y_0: u32 = row;
        while y_0 < h {
            let mut x_0: u32 = 0 as i32 as u32;
            while x_0 < w {
                let data_0: *mut libc::c_uchar = (*bmp)
                    .pixels
                    .offset(stride.wrapping_mul(y_0) as isize)
                    .offset(x_0.wrapping_mul(ch) as isize);
                let r_0: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(0) * *data_0.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(0) * *data_0.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(0) * *data_0.offset(0) as i32 as f32
                        + m40,
                );
                let g_0: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(1) * *data_0.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(1) * *data_0.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(1) * *data_0.offset(0) as i32 as f32
                        + m41,
                );
                let b_0: u8 = uchar_clamp_ff(
                    *(*m.offset(0)).offset(2) * *data_0.offset(2) as i32 as f32
                        + *(*m.offset(1)).offset(2) * *data_0.offset(1) as i32 as f32
                        + *(*m.offset(2)).offset(2) * *data_0.offset(0) as i32 as f32
                        + m42,
                );
                let newdata_0: *mut u8 = (*bmp)
                    .pixels
                    .offset(stride.wrapping_mul(y_0) as isize)
                    .offset(x_0.wrapping_mul(ch) as isize);
                *newdata_0.offset(0) = b_0;
                *newdata_0.offset(1) = g_0;
                *newdata_0.offset(2) = r_0;
                x_0 = x_0.wrapping_add(1)
            }
            y_0 = y_0.wrapping_add(1)
        }
    } else {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Unsupported_pixel_format,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1838 as i32,
            (*::std::mem::transmute::<&[u8; 36], &[libc::c_char; 36]>(
                b"flow_bitmap_bgra_apply_color_matrix\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    return true;
}
// note: this file isn't exercised by test suite
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_float_apply_color_matrix(
    context: *mut flow_c,
    bmp: *mut flow_bitmap_float,
    row: u32,
    count: u32,
    m: *mut *mut f32,
) -> bool {
    let stride: u32 = (*bmp).float_stride;
    let ch: u32 = (*bmp).channels;
    let w: u32 = (*bmp).w;
    let h: u32 = row.wrapping_add(count).min((*bmp).h);
    match ch {
        4 => {
            let mut y: u32 = row;
            while y < h {
                let mut x: u32 = 0 as i32 as u32;
                while x < w {
                    let data: *mut f32 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y) as isize)
                        .offset(x.wrapping_mul(ch) as isize);
                    let r: f32 = *(*m.offset(0)).offset(0) * *data.offset(2)
                        + *(*m.offset(1)).offset(0) * *data.offset(1)
                        + *(*m.offset(2)).offset(0) * *data.offset(0)
                        + *(*m.offset(3 as i32 as isize)).offset(0)
                        * *data.offset(3 as i32 as isize)
                        + *(*m.offset(4 as i32 as isize)).offset(0);
                    let g: f32 = *(*m.offset(0)).offset(1) * *data.offset(2)
                        + *(*m.offset(1)).offset(1) * *data.offset(1)
                        + *(*m.offset(2)).offset(1) * *data.offset(0)
                        + *(*m.offset(3 as i32 as isize)).offset(1)
                        * *data.offset(3 as i32 as isize)
                        + *(*m.offset(4 as i32 as isize)).offset(1);
                    let b: f32 = *(*m.offset(0)).offset(2) * *data.offset(2)
                        + *(*m.offset(1)).offset(2) * *data.offset(1)
                        + *(*m.offset(2)).offset(2) * *data.offset(0)
                        + *(*m.offset(3 as i32 as isize)).offset(2)
                        * *data.offset(3 as i32 as isize)
                        + *(*m.offset(4 as i32 as isize)).offset(2);
                    let a: f32 = *(*m.offset(0)).offset(3 as i32 as isize) * *data.offset(2)
                        + *(*m.offset(1)).offset(3 as i32 as isize) * *data.offset(1)
                        + *(*m.offset(2)).offset(3 as i32 as isize) * *data.offset(0)
                        + *(*m.offset(3 as i32 as isize)).offset(3 as i32 as isize)
                        * *data.offset(3 as i32 as isize)
                        + *(*m.offset(4 as i32 as isize)).offset(3 as i32 as isize);
                    let newdata: *mut f32 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y) as isize)
                        .offset(x.wrapping_mul(ch) as isize);
                    *newdata.offset(0) = b;
                    *newdata.offset(1) = g;
                    *newdata.offset(2) = r;
                    *newdata.offset(3 as i32 as isize) = a;
                    x = x.wrapping_add(1)
                }
                y = y.wrapping_add(1)
            }
            return true;
        }
        3 => {
            let mut y_0: u32 = row;
            while y_0 < h {
                let mut x_0: u32 = 0 as i32 as u32;
                while x_0 < w {
                    let data_0: *mut f32 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y_0) as isize)
                        .offset(x_0.wrapping_mul(ch) as isize);
                    let r_0: f32 = *(*m.offset(0)).offset(0) * *data_0.offset(2)
                        + *(*m.offset(1)).offset(0) * *data_0.offset(1)
                        + *(*m.offset(2)).offset(0) * *data_0.offset(0)
                        + *(*m.offset(4 as i32 as isize)).offset(0);
                    let g_0: f32 = *(*m.offset(0)).offset(1) * *data_0.offset(2)
                        + *(*m.offset(1)).offset(1) * *data_0.offset(1)
                        + *(*m.offset(2)).offset(1) * *data_0.offset(0)
                        + *(*m.offset(4 as i32 as isize)).offset(1);
                    let b_0: f32 = *(*m.offset(0)).offset(2) * *data_0.offset(2)
                        + *(*m.offset(1)).offset(2) * *data_0.offset(1)
                        + *(*m.offset(2)).offset(2) * *data_0.offset(0)
                        + *(*m.offset(4 as i32 as isize)).offset(2);
                    let newdata_0: *mut f32 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y_0) as isize)
                        .offset(x_0.wrapping_mul(ch) as isize);
                    *newdata_0.offset(0) = b_0;
                    *newdata_0.offset(1) = g_0;
                    *newdata_0.offset(2) = r_0;
                    x_0 = x_0.wrapping_add(1)
                }
                y_0 = y_0.wrapping_add(1)
            }
            return true;
        }
        _ => {
            flow_context_set_error_get_message_buffer(
                context,
                flow_status_code::Unsupported_pixel_format,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1893 as i32,
                (*::std::mem::transmute::<&[u8; 37], &[libc::c_char; 37]>(
                    b"flow_bitmap_float_apply_color_matrix\x00",
                ))
                    .as_ptr(),
            );
            return false;
        }
    };
}
#[no_mangle]
pub unsafe extern "C" fn flow_bitmap_bgra_populate_histogram(
    context: *mut flow_c,
    bmp: *const flow_bitmap_bgra,
    histograms: *mut u64,
    histogram_size_per_channel: u32,
    histogram_count: u32,
    pixels_sampled: *mut u64,
) -> bool {
    let row: u32 = 0;
    let count: u32 = (*bmp).h;
    let stride: u32 = (*bmp).stride;
    let ch: u32 = flow_pixel_format_bytes_per_pixel((*bmp).fmt);
    let w: u32 = (*bmp).w;
    let h: u32 = (row.wrapping_add(count)).min((*bmp).h);
    if histogram_size_per_channel != 256 {
        // We're restricting it to this for speed
        FLOW_error(
            context,
            flow_status_code::Invalid_argument,
            "flow_bitmap_bgra_populate_histogram",
        );
        return false;
    }
    let shift = 0; // 8 - intlog2(histogram_size_per_channel);
    if ch == 4 || ch == 3 {
        if histogram_count == 1 {
            let mut y: u32 = row;
            while y < h {
                let mut x: u32 = 0;
                while x < w {
                    let data: *const u8 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y) as isize)
                        .offset(x.wrapping_mul(ch) as isize);
                    let ref mut fresh9 = *histograms.offset(
                        (306 as i32 * *data.offset(2) as i32
                            + 601 as i32 * *data.offset(1) as i32
                            + 117 as i32 * *data.offset(0) as i32
                            >> shift) as isize,
                    );
                    *fresh9 = (*fresh9).wrapping_add(1);
                    x = x.wrapping_add(1)
                }
                y = y.wrapping_add(1)
            }
        } else if histogram_count == 3 {
            let mut y: u32 = row;
            while y < h {
                let mut x: u32 = 0;
                while x < w {
                    let data: *const u8 = (*bmp)
                        .pixels
                        .offset((stride * y) as isize)
                        .offset((x * ch) as isize);
                    let ref mut fresh10 =
                        *histograms.offset((*data.offset(2) as i32 >> shift) as isize);
                    *fresh10 = (*fresh10).wrapping_add(1);
                    let ref mut fresh11 = *histograms.offset(
                        ((*data.offset(1) as i32 >> shift) as u32)
                            .wrapping_add(histogram_size_per_channel)
                            as isize,
                    );
                    *fresh11 = (*fresh11).wrapping_add(1);
                    let ref mut fresh12 = *histograms.offset(
                        ((*data.offset(0) as i32 >> shift) as u32)
                            .wrapping_add((2u32).wrapping_mul(histogram_size_per_channel))
                            as isize,
                    );
                    *fresh12 = (*fresh12).wrapping_add(1);
                    x = x.wrapping_add(1)
                }
                y = y.wrapping_add(1)
            }
        } else if histogram_count == 2 {
            let mut y_1: u32 = row;
            while y_1 < h {
                let mut x_1: u32 = 0 as i32 as u32;
                while x_1 < w {
                    let data_1: *const u8 = (*bmp)
                        .pixels
                        .offset(stride.wrapping_mul(y_1) as isize)
                        .offset(x_1.wrapping_mul(ch) as isize);
                    // Calculate luminosity and saturation
                    let ref mut fresh13 = *histograms.offset(
                        (306 as i32 * *data_1.offset(2) as i32
                            + 601 as i32 * *data_1.offset(1) as i32
                            + 117 as i32 * *data_1.offset(0) as i32
                            >> shift) as isize,
                    );
                    *fresh13 = (*fresh13).wrapping_add(1);
                    let ref mut fresh14 =
                        *histograms.offset(histogram_size_per_channel.wrapping_add(
                            (int_max(
                                255 as i32,
                                int_max(
                                    (*data_1.offset(2) as i32 - *data_1.offset(1) as i32).abs(),
                                    (*data_1.offset(1) as i32 - *data_1.offset(0) as i32).abs(),
                                ),
                            ) >> shift) as u32,
                        ) as isize);
                    *fresh14 = (*fresh14).wrapping_add(1);
                    x_1 = x_1.wrapping_add(1)
                }
                y_1 = y_1.wrapping_add(1)
            }
        } else {
            flow_context_set_error_get_message_buffer(
                context,
                flow_status_code::Invalid_internal_state,
                b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
                1950 as i32,
                (*::std::mem::transmute::<&[u8; 36], &[libc::c_char; 36]>(
                    b"flow_bitmap_bgra_populate_histogram\x00",
                ))
                    .as_ptr(),
            );
            return false;
        }
        *pixels_sampled = h.wrapping_sub(row).wrapping_mul(w) as u64
    } else {
        flow_context_set_error_get_message_buffer(
            context,
            flow_status_code::Unsupported_pixel_format,
            b"lib/graphics.c\x00" as *const u8 as *const libc::c_char,
            1956 as i32,
            (*::std::mem::transmute::<&[u8; 36], &[libc::c_char; 36]>(
                b"flow_bitmap_bgra_populate_histogram\x00",
            ))
                .as_ptr(),
        );
        return false;
    }
    return true;
}
// Gamma correction  http://www.4p8.com/eric.brasseur/gamma.html#formulas
#[no_mangle]
pub unsafe extern "C" fn flow_colorcontext_init(
    _context: *mut flow_c,
    mut colorcontext: *mut flow_colorcontext_info,
    space: flow_working_floatspace,
    a: f32,
    _b: f32,
    _c: f32,
) {
    (*colorcontext).floatspace = space;
    (*colorcontext).apply_srgb = (space & flow_working_floatspace_linear) > 0;
    (*colorcontext).apply_gamma = (space & flow_working_floatspace_gamma) > 0;
    /* Code guarded by #ifdef EXPOSE_SIGMOID not translated */
    if (*colorcontext).apply_gamma {
        (*colorcontext).gamma = a;
        (*colorcontext).gamma_inverse = (1.0f64 / a as f64) as f32
    }
    for n in 0..256 {
        (*colorcontext).byte_to_float[n] =
            flow_colorcontext_srgb_to_floatspace_uncached(colorcontext, n as u8);
    }
}
