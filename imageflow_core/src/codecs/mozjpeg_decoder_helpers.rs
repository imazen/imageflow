use std;
use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::ffi::BitmapBgra;
use super::*;
extern crate mozjpeg_sys;
use ::mozjpeg_sys::*;

const ICC_OVERHEAD_LEN: u32 = 14; /* size of non-profile data in APP2 */


fn is_marker_icc(marker: &mozjpeg_sys::jpeg_marker_struct) -> bool{
    // verify the identifying string
    let is_icc = marker.marker == ffi::JpegMarker::ICC as u8 &&
        marker.data_length >= ICC_OVERHEAD_LEN &&
        unsafe{ std::slice::from_raw_parts(marker.data, marker.data_length as usize) }
            .starts_with(b"ICC_PROFILE\0");
    is_icc
}

/// Reassemble and return the profile data.
/// If the file contains invalid ICC APP2 markers we return None
pub fn read_icc_profile(codec: &mozjpeg_sys::jpeg_decompress_struct) -> Option<Vec<u8>>{

    let mut num_markers = 0usize;
    const MAX_MARKER_COUNT:usize = 256;
    let mut marker_present: [bool;MAX_MARKER_COUNT] = [false; MAX_MARKER_COUNT];
    let mut data_length: [usize;MAX_MARKER_COUNT] = [0; MAX_MARKER_COUNT];
    let mut data_offset: [usize;MAX_MARKER_COUNT] = [0; MAX_MARKER_COUNT];

    // Verify all ICC segments consistently report the same segment count
    // Verify all ICC segment indices are in range and there are no duplicates
    // Record data lengths
    let mut current_marker = codec.marker_list;
    while !current_marker.is_null(){
        let current_marker_ref = unsafe{&*current_marker};
        if is_marker_icc(current_marker_ref){
            let data = unsafe{ std::slice::from_raw_parts(current_marker_ref.data,
                                                          current_marker_ref.data_length as usize) };
            if num_markers == 0 {
                num_markers = data[13] as usize;
            }else if num_markers != data[13] as usize {
                /* inconsistent num_markers fields */
                return None;
            }

            let seq_no = data[12] as usize;
            if seq_no == 0 || seq_no > num_markers{
                /* bogus sequence number */
                return None;
            }
            if marker_present[seq_no] {
                /* duplicate sequence numbers */
                return None;
            }
            marker_present[seq_no] = true;

            data_length[seq_no] = (current_marker_ref.data_length - ICC_OVERHEAD_LEN) as usize;
        }
        current_marker = current_marker_ref.next;
    }
    // Verify there are ICC segments
    if num_markers == 0{
        // No ICC segments
        return None
    }

    // Check for missing segments and record offsets
    let mut total_length = 0;
    for seq_no in 1..=num_markers{
        if !marker_present[seq_no]{
            // Missing sequence number
            return None
        }
        data_offset[seq_no] = total_length;
        total_length += data_length[seq_no];
    }
    if total_length <= 0{
        // Found only empty markers
        return None
    }

    let mut reassembled_data =vec![0; total_length as usize];


    let mut current_marker = codec.marker_list;
    while !current_marker.is_null(){
        let current_marker_ref = unsafe{&*current_marker};
        if is_marker_icc(current_marker_ref){
            let data = unsafe{ std::slice::from_raw_parts(current_marker_ref.data,
                                                          current_marker_ref.data_length as usize) };
            let seq_no = data[12] as usize;


            reassembled_data[data_offset[seq_no]..data_offset[seq_no]+data_length[seq_no]]
                .copy_from_slice(&data[(ICC_OVERHEAD_LEN as usize)..]);
        }
        current_marker = current_marker_ref.next;
    }

    Some(reassembled_data)
}
