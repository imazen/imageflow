use blake2_rfc::blake2b::{ blake2b};
use std;
use crate::preludes::from_std::*;
use std::path::MAIN_SEPARATOR;
use regex::{Regex,Captures};
use twox_hash::XxHash;
use std::hash::Hasher;

// Guidance for selecting a hash function
// Need cryptographic? Use black2b 256 or 512 and a secret seed.
// Don't fold or truncate cryptographic results; they're no longer cryptographic, and you'll get collisions.
// Need fast for short keys without nulls? FNV1a. Like djb2, it can zero out, so avoid null bytes or sequences which can cause a sticky state.
// Need fast for huge arrays? metrohash or xxhash.
//

/// This is an extremely fast non-cryptographic hash
pub fn hash_64(bytes: &[u8]) -> u64{
    // But metrohash is 20% faster and can do 128bit
    // https://github.com/arthurprs/metrohash-rs
    // So maybe we should consider that...
    let mut h = XxHash::with_seed(0x8ed1_2ad9_483d_28a0);
    h.write(bytes);
    h.finish()
}

///
/// Returns a 32-byte cryptographic hash of the given data (via Blake2b), with a null seed.
pub fn hash_256(bytes: &[u8]) -> [u8;32]{
    let mut hash: [u8;32] = [0u8;32];
    hash256_to(bytes, &mut hash);
    hash
}
fn hash256_to(bytes: &[u8], to: &mut [u8;32]){
    to.copy_from_slice(blake2b(32, &[], bytes).as_bytes());
}

pub fn legacy_djb2(bytes: &[u8]) -> u64{
    bytes.iter().fold(5381u64, |hash, c| ((hash << 5).wrapping_add(hash)).wrapping_add(u64::from(*c)))
}

/// Format string supports printing specific bit ranges in hexadecimal:
/// `{0-23:x}` will print the first 23 bits of the hash in hex - WITHOUT leading zeroes
/// `{0-256:064x}` will print all 256 bits, padded to 64 hex digits (256 bits) with zeroes
///
/// No escaping of `{` is supported.
pub fn bits_format(bits: &[u8], format: &'static str) -> String{
    lazy_static! {
      static ref RE: Regex = Regex::new(r"\{([0-9]+)-([0-9]+):(0([0-9]+))?x\}").unwrap();
    };

    RE.replace_all(format, |c: &Captures | {
        let from = c[1].parse::<usize>().unwrap();
        let until = c[2].parse::<usize>().unwrap();
        let padding = c.get(4).and_then(|f| Some(f.as_str().parse::<usize>().unwrap_or(0))).unwrap_or(0);
        if from == 0 && until == bits.len() * 8{
            format!("{:01$x}", HexableBytes(bits), padding)
        }else{
            format!("{:01$x}", bits_select(bits, from, until).expect("Formats may specify up to 57 bits or the entire range, but no range greater than 58 and less than the the whole"), padding)
        }
    }).into_owned()
}
///
/// Returns up to 57 bits from the provided byte slice, using big-endian interpretation.
pub fn bits_select(hash: &[u8], from: usize, until: usize) -> Option<u64>{
    if until > hash.len() * 8 || until < from || until - from > 57 {
        return None;
    }
    let relevant_bytes = &hash[from / 8..(until + 7) / 8];
    let truncate_right = (8 - until % 8) % 8;
    let mask = if until == from { 0 } else{ !0u64 >> (64 - (until - from)) };

    Some((relevant_bytes.iter().fold(0u64, | acc, elem| (u64::from(*elem) | (acc << 8)) ) >> truncate_right) & mask)
    //println!("bits {} to {} of {:032x} - using bytes {} to {}. Unshift {} and mask {:#016x} to produce {:x}",from, until, HexableBytes(hash), (from / 8), (until + 7) / 8, truncate_right, mask, res.unwrap());
}

/// Prints the bytes as hex, padding with zeroes
pub fn bytes_to_hex(bytes: &[u8]) -> String{
    format!("{:01$x}", HexableBytes(bytes), bytes.len() * 2)
}

/// Behavior undefined on platforms where the directory separator is not / or \
pub fn normalize_slashes(s: String) -> String {
    if MAIN_SEPARATOR == '/' && s.contains('\\') {
        s.replace("\\", "/")
    } else if MAIN_SEPARATOR == '\\' && s.contains('/') {
        s.replace("/", "\\")
    }else{
        s
    }
}

pub struct HexableBytes<'a>(pub &'a [u8]);

//impl<'a> HexableBytes<'a>{
//    pub fn wrap(bytes: &[u8]) -> HexableBytes {
//        HexableBytes(bytes)
//    }
//}

impl<'a> std::fmt::LowerHex for HexableBytes<'a> {
    fn fmt(&self, fmtr: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let mut nonzero = false;
        for (index, byte) in self.0.iter().enumerate() {
            if nonzero {
                fmtr.write_fmt(format_args!("{:02x}", byte))?;
            }else if *byte > 0{
                let pad_width = fmtr.width().and_then(|w| {
                    let a = 2 + w as i64 - (self.0.len() as i64 - index as i64) * 2;
                    if a < 2 {
                        None //At least one character will be written anyway.
                    }else{
                        Some(a)
                    }
                }).unwrap_or(0);
                fmtr.write_fmt(format_args!("{:01$x}", byte, pad_width as usize))?;
                nonzero = true;
            }
        }
        Ok(())
    }
}




#[test]
fn test_bits_format(){
    let hash = hash_256(b"perplexities");
    assert_eq!("dbf90c29d914a7e3b0756e3365e87cf05723a7df53c01dcebda066ce7a99488c", bytes_to_hex(&hash));
    assert_eq!("/c/8/8c/88/dbf90c29d914a7e3b0756e3365e87cf05723a7df53c01dcebda066ce7a99488c", bits_format(&hash, "/{252-256:x}/{248-252:x}/{248-256:x}/{244-252:x}/{0-256:064x}"));
    assert_eq!("/0c/22/4/dbf90c29d914a7e3b0756e3365e87cf05723a7df53c01dcebda066ce7a99488c", bits_format(&hash, "/{250-256:02x}/{244-250:02x}/{240-244:x}/{0-256:x}"));
}

#[test]
fn test_bits_select(){
    assert_eq!(!0u64 >> 63, 1);
    assert_eq!(None, bits_select(&[1u8;32],0, 58));
    assert_eq!(None, bits_select(&[1u8;32],240, 298));
    assert_eq!(Some(1), bits_select(&[255u8;32],255, 256));
    assert_eq!(Some(3), bits_select(&[255u8;32],254, 256));
    assert_eq!(Some(2u64.pow(57) - 1), bits_select(&[255u8;32],0, 57));
    assert_eq!(Some(2u64.pow(11) - 1), bits_select(&[255u8;32],245, 256));
}

#[test]
fn compare_byte_styles(){
    let bytes = [0, 0, 2, 1, 5, 255, 32];
    assert_eq!("20105ff20", format!("{:x}", HexableBytes(&bytes)));
    assert_eq!("20105ff20", format!("{:09x}", HexableBytes(&bytes)));
    assert_eq!("20105ff20", format!("{:08x}", HexableBytes(&bytes)));
    assert_eq!("020105ff20", format!("{:10x}", HexableBytes(&bytes)));
    assert_eq!("0020105ff20", format!("{:11x}", HexableBytes(&bytes)));
    assert_eq!("20105ff20", format!("{:x}", HexableBytes(&bytes)));
    assert_eq!("0000020105ff20", bytes_to_hex(&bytes));
    assert_eq!(bytes_to_hex(&bytes), format!("{:01$x}", HexableBytes(&bytes), bytes.len() * 2));
}
