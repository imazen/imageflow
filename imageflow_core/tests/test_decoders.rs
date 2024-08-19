use zune_bmp::zune_core::bytestream::ZCursor;

// #[test]
// pub fn test_rgba32_bmp_channel_order(){
//     let bytes32 = imageflow_http_helpers::fetch_bytes("https://raw.githubusercontent.com/etemesi254/zune-image/dev/test-images/bmp/rgba32-1.bmp").unwrap();
//     let bytes24 = imageflow_http_helpers::fetch_bytes("https://raw.githubusercontent.com/etemesi254/zune-image/dev/test-images/bmp/rgb24.bmp").unwrap();
//
//     let mut decoder32 = zune_bmp::BmpDecoder::new(ZCursor::new(&bytes32));
//     let decoded_bytes = decoder32.decode().unwrap();
//     // We know the first pixel is more red than blue, so RGB order means byte 0 is greater than byte 2
//     assert!(decoded_bytes[0] > decoded_bytes[2]);
//     // The above passes
//
//     let mut decoder24 = zune_bmp::BmpDecoder::new(ZCursor::new(&bytes24));
//     let decoded_bytes = decoder24.decode().unwrap();
//     // We know the first pixel is more red than blue, so RGB order means byte 0 is greater than byte 2
//     assert!(decoded_bytes[0] > decoded_bytes[2]);
//     // But this one fails.
// }