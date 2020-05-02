
extern crate libc;
extern crate imageflow_types as s;
extern crate imageflow_helpers as hlp;
extern crate serde_json;
extern crate dssim;
extern crate rgb;

extern crate itertools;
extern crate twox_hash;
extern crate imgref;

use std::ffi::CString;
use std::path::Path;
use imageflow_core::{Context};

use imageflow_core::ffi::BitmapBgra;
use std::collections::BTreeMap;
use std::fs::File;
use std::path::{PathBuf};
use std::io::Write;
use std;
use imageflow_core;

use std::sync::RwLock;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ChecksumMatch {
    Match,
    Mismatch,
    NewStored,
}

/// Executes the given steps (adding a frame buffer container to the end of them).
/// Returns the width and height of the resulting frame.
/// Steps must be open-ended - they cannot be terminated with an encoder.
pub fn get_result_dimensions(steps: &[s::Node], io: Vec<s::IoObject>, debug: bool) -> (u32, u32) {
    let mut bit = BitmapBgraContainer::empty();
    let mut steps = steps.to_vec();
    steps.push(unsafe { bit.get_node() });

    let build = s::Build001{
        builder_config: Some(default_build_config(debug)),
        io,
        framewise: s::Framewise::Steps(steps)
    };
    let mut context = Context::create().unwrap();
    let result = context.build_1(build).unwrap();

    if let Some(b) = unsafe { bit.bitmap(&context) } {
        (b.w, b.h)
    }else{
        panic!("execution failed: {:?}", result);
    }
}

/// Just validates that no errors are thrown during job execution
pub fn smoke_test(input: Option<s::IoEnum>, output: Option<s::IoEnum>,  debug: bool, steps: Vec<s::Node>) -> Result<s::ResponsePayload, imageflow_core::FlowError>{
    let mut io_list = Vec::new();
    if input.is_some() {
        io_list.push(s::IoObject {
            io_id: 0,
            direction: s::IoDirection::In,

            io: input.unwrap()
        });
    }
    if output.is_some() {
        io_list.push(s::IoObject {
            io_id: 1,
            direction: s::IoDirection::Out,

            io: output.unwrap()
        });
    }
    let build = s::Build001{
        builder_config: Some(default_build_config(debug)),
        io: io_list,
        framewise: s::Framewise::Steps(steps)
    };
    let mut context = Context::create().unwrap();
    context.build_1(build)
}


/// A context for getting/storing frames and frame checksums by test name.
/// Currently has read-only support for remote storage.
/// TODO: Add upload support; it's very annoying to do it manually
pub struct ChecksumCtx<'a>{
    c: &'a Context,
    checksum_file: PathBuf,
    url_list_file: PathBuf,
    visuals_dir: PathBuf,
    #[allow(dead_code)]
    cache_dir: PathBuf,
    create_if_missing: bool,
    url_base: &'static str
}

lazy_static! {
    static ref CHECKSUM_FILE: RwLock<()> = RwLock::new(());
}

impl<'a> ChecksumCtx<'a>{

    /// A checksum context configured for tests/visuals/*
    pub fn visuals(c: &Context) -> ChecksumCtx{
        let visuals = Path::new(env!("CARGO_MANIFEST_DIR")).join(Path::new("tests")).join(Path::new("visuals"));
        std::fs::create_dir_all(&visuals).unwrap();
        ChecksumCtx {
            c,
            visuals_dir: visuals.clone(),
            cache_dir: visuals.join(Path::new("cache")),
            create_if_missing: true,
            checksum_file: visuals.join(Path::new("checksums.json")),
            url_list_file:  visuals.join(Path::new("images.txt")),
            url_base: "https://s3-us-west-2.amazonaws.com/imageflow-resources/visual_test_checksums/"
        }
    }

    /// Load the checksum map
    fn load_list(&self) -> Result<BTreeMap<String,String>,()>{
        if self.checksum_file.exists() {
            let map: BTreeMap<String, String> = ::serde_json::from_reader(::std::fs::File::open(&self.checksum_file).unwrap()).unwrap();
            Ok(map)
        }else{
            Ok(BTreeMap::new())
        }
    }

    /// Save the checksum map and url_list to disk
    fn save_list(&self, map: &BTreeMap<String,String>) -> Result<(),()>{
        let mut f = ::std::fs::File::create(&self.checksum_file).unwrap();
        ::serde_json::to_writer_pretty(&mut f, map).unwrap();
        f.sync_all().unwrap();
        // Write the URL list
        // We can use this to prefetch required images in the background on CI)
        // TODO: add simple script to do this
        let mut f = ::std::fs::File::create(&self.url_list_file).unwrap();
        use self::itertools::Itertools;
        let list_contents = map.values().map(|key| self.image_url(key)).join("\n");
        f.write_all(list_contents.as_bytes()).unwrap();
        f.sync_all().unwrap();
        Ok(())
    }


    /// Get the stored result checksum for a named test
    #[allow(unused_variables)]
    pub fn get(&self, name: &str) -> Option<String>{
        #[allow(unused_variables)]
        let lock = CHECKSUM_FILE.read().unwrap();
        self.load_list().unwrap().get(name).and_then(|v|Some(v.to_owned()))
    }

    /// Set the result checksum for a named test
    #[allow(unused_variables)]
    pub fn set(&self, name: String, checksum: String) -> Result<(),()>{
        #[allow(unused_variables)]
        let lock = CHECKSUM_FILE.write().unwrap();
        let mut map = self.load_list().unwrap();
        map.insert(name,checksum);
        self.save_list(&map).unwrap();
        Ok(())
    }


    pub fn image_url(&self, checksum: &str) -> String{
        if !checksum.contains("."){
            format!("{}{}.png",self.url_base, checksum)
        }else{
            format!("{}{}", self.url_base, checksum)
        }
    }

    pub fn image_path(&self, checksum: &str) -> PathBuf{
        let name = if !checksum.contains("."){
            format!("{}.png", checksum)
        }else{
            format!("{}", checksum)
        };

        self.visuals_dir.as_path().join(Path::new(&name))
    }

    pub fn image_path_string(&self, checksum: &str) -> String{
        self.image_path(checksum).into_os_string().into_string().unwrap()
    }
    pub fn image_path_cstring(&self, checksum: &str) -> CString{
        CString::new(self.image_path_string(checksum)).unwrap()
    }
    /// Fetch the given image to disk
    pub fn fetch_image(&self, checksum: &str){
        let dest_path = self.image_path(checksum);
        let source_url = self.image_url(checksum);
        if dest_path.exists() {
            println!("{} (trusted) exists", checksum);
        }else{
            println!("Fetching {} to {:?}", &source_url, &dest_path);
            let bytes = hlp::fetching::fetch_bytes(&source_url).expect("Did you forget to upload {} to s3?");
            File::create(&dest_path).unwrap().write_all(bytes.as_ref()).unwrap();
        }
    }

    /// Load the given image from disk (and download it if it's not on disk)
    /// The bitmap will be destroyed when the returned Context goes out of scope
    pub fn load_image(&self, checksum: &str) -> (Box<Context>, *mut BitmapBgra) {
        self.fetch_image(checksum);

        let mut c = Context::create().unwrap();
        let path = self.image_path_string(checksum);
        c.add_file(0, s::IoDirection::In, &path).unwrap();

        let image =  decode_image(&mut *c, 0) as *mut BitmapBgra;
        (c, image)
    }


    /// Save the given image to disk by calculating its checksum.
    pub fn save_frame(&self, bit: &BitmapBgra, checksum: &str){
        let dest_path = self.image_path(&checksum);
        if !dest_path.exists(){
            let dest_cpath = self.image_path_cstring(&checksum);
            let path_str = dest_path.to_str();
            if let Some(path) = path_str{
                println!("Writing {}", &path);
            }else {
                println!("Writing {:#?}", &dest_path);
            }
            unsafe {
                if !::imageflow_core::ffi::flow_bitmap_bgra_save_png(self.c.flow_c(), bit as *const BitmapBgra, dest_cpath.as_ptr()){
                    cerror!(self.c).panic();
                }
            }
        }
    }
    /// Save the given bytes to disk by calculating their checksum.
    pub fn save_bytes(&self, bytes: &[u8], checksum: &str){
        let dest_path = self.image_path(&checksum);
        if !dest_path.exists(){
            println!("Writing {:?}", &dest_path);
            let mut f = ::std::fs::File::create(&dest_path).unwrap();
            f.write_all(bytes).unwrap();
            f.sync_all().unwrap();
        }
    }

    /// We include the file extension in checksums of encoded images, as we can't be sure they're stored as PNG (as we can with frame checksums)
    pub fn checksum_bytes(bytes: &[u8]) -> String {
        format!("{:02$X}.{1}", hlp::hashing::hash_64(bytes), Self::file_extension_for_bytes(bytes), 17)
    }

    pub fn file_extension_for_bytes(bytes: &[u8]) -> &'static str{
        if bytes.starts_with(&[0x89,0x50,0x4E,0x47,0x0D,0x0A,0x1A,0x0A]){
            "png"
        }else if bytes.starts_with(b"GIF8"){
            "gif"
        }else if bytes.starts_with(&[0xFF,0xD8,0xFF]) {
            "jpg"
        } else{
            "unknown"
        }
    }

    /// Provides a checksum composed of two hashes - one from the pixels, one from the dimensions and format
    /// This format is preserved from legacy C tests, thus its rudimentary (but, I suppose, sufficient) nature.
    pub fn checksum_bitmap(bitmap: &BitmapBgra) -> String {
        let info = format!("{}x{} fmt={}", bitmap.w, bitmap.h, bitmap.fmt as i32);
        return format!("{:02$X}_{:02$X}", bitmap.short_hash_pixels(), hlp::hashing::legacy_djb2(info.as_bytes()), 17)
    }


    /// Checksums the result, saves it to disk, the compares the actual checksum to the expected checksum.
    ///
    /// Complains loudly and returns false if the checksums don't match. Also returns the trusted checksum.
    ///
    /// if there is no trusted checksum, create_if_missing is set, then
    /// the checksum will be stored, and the function will return true.
    pub fn bitmap_matches(&self, bitmap: &mut BitmapBgra, name: &str) -> (ChecksumMatch, String){
        bitmap.normalize_alpha().unwrap();

        let actual = Self::checksum_bitmap(bitmap);
        //println!("actual = {}", &actual);
        // Always write a copy if it doesn't exist
        self.save_frame(bitmap, &actual);

        self.exact_match(actual, name)
    }

    /// Checksums the result, saves it to disk, the compares the actual checksum to the expected checksum.
    ///
    /// Complains loudly and returns false if the checksums don't match. Also returns the trusted checksum.
    ///
    /// if there is no trusted checksum, create_if_missing is set, then
    /// the checksum will be stored, and the function will return true.
    pub fn bytes_match(&self, bytes: &[u8], name: &str) -> (ChecksumMatch, String){
        let actual = Self::checksum_bytes(bytes);

        //println!("actual = {}", &actual);

        // Always write a copy if it doesn't exist
        self.save_bytes(bytes, &actual);

        self.exact_match(actual, name)
    }


    /// Compares the actual checksum to the expected checksum. Returns the trusted checksum.
    ///
    /// Complains loudly and returns false if the checksums don't match.
    ///
    /// if there is no trusted checksum, create_if_missing is set, then
    /// the checksum will be stored, and the function will return true.
    pub fn exact_match(&self, actual_checksum: String, name: &str) -> (ChecksumMatch, String){
        if let Some(trusted) = self.get(name){
            if trusted == actual_checksum{
                (ChecksumMatch::Match, trusted)
            }else{
                println!("====================\n{}\nThe stored checksum {} differs from the actual_checksum one {}\nTrusted: {}\nActual: {}\n",
                         name, &trusted,
                         &actual_checksum,
                         self.image_path(&trusted).to_str().unwrap(),
                         self.image_path(&actual_checksum).to_str().unwrap());
                (ChecksumMatch::Mismatch, trusted)
            }
        }else{
            if self.create_if_missing {
                println!("====================\n{}\nStoring checksum {}", name, &actual_checksum);
                self.set(name.to_owned(), actual_checksum.clone()).unwrap();
                (ChecksumMatch::NewStored, actual_checksum)
            } else {
                panic!("There is no stored checksum for {}; rerun with create_if_missing=true", name);
            }
        }
    }

    // TODO: implement uploader
}

pub fn decode_image(c: &mut Context, io_id: i32) -> &mut BitmapBgra {
    let mut bit = BitmapBgraContainer::empty();
    let _result = c.execute_1(s::Execute001 {
        graph_recording: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode {
                io_id,
                commands: None
            },
            unsafe { bit.get_node() }
        ])
    }).unwrap();
    unsafe{ bit.bitmap(c).unwrap() }
}

pub fn decode_input(c: &mut Context, input: s::IoEnum) -> &mut BitmapBgra {
    let mut bit = BitmapBgraContainer::empty();
    let _result = c.build_1(s::Build001 {
        builder_config: None,
        io: vec![input.into_input(0)
        ],
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode {
                io_id: 0,
                commands: None
            },
            unsafe { bit.get_node() }
        ])
    }).unwrap();
    unsafe { bit.bitmap(c).unwrap() }
}




/// Returns the number of bytes that differ, followed by the total value of all differences
/// If these are equal, then only off-by-one errors are occurring
fn diff_bytes(a: &[u8], b: &[u8]) ->(i64,i64){
    a.iter().zip(b.iter()).fold((0,0), |(count, delta), (a,b)| if a != b { (count + 1, delta + (i64::from(*a) - i64::from(*b)).abs()) } else { (count,delta)})
}

///
/// Likely a slow spot. Returns the number of bytes that differ, followed by the sum of the absolute value of each difference.
///
fn diff_bitmap_bytes(a: &BitmapBgra, b: &BitmapBgra) -> (i64,i64){
    if a.w != b.w || a.h != b.h || a.fmt.bytes() != b.fmt.bytes() {
        panic!("Bitmap dimensions differ. a:\n{:#?}\nb:\n{:#?}", a, b);
    }

    let width_bytes = a.w as usize * a.fmt.bytes();
    (0isize..a.h as isize).map(|h| {

        let a_contents_slice = unsafe { ::std::slice::from_raw_parts(a.pixels.offset(h * a.stride as isize), width_bytes) };
        let b_contents_slice = unsafe { ::std::slice::from_raw_parts(b.pixels.offset(h * b.stride as isize), width_bytes) };

        if a_contents_slice == b_contents_slice {
            (0, 0)
        }else {
            diff_bytes(a_contents_slice, b_contents_slice)
        }

    }).fold((0, 0), |(a, b), (c, d)| (a + c, b + d))
}



#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Similarity{
    AllowOffByOneBytesCount(i64),
    AllowOffByOneBytesRatio(f32),
    AllowDssimMatch(f64, f64)
}

impl Similarity{
    fn report_on_bytes(&self, count: i64, delta: i64, len: usize) -> Option<String>{

        let allowed_off_by_one_bytes: i64 = match *self {
            Similarity::AllowOffByOneBytesCount(v) => v,
            Similarity::AllowOffByOneBytesRatio(ratio) => (ratio * len as f32) as i64,
            Similarity::AllowDssimMatch(..) => return None,
        };
        eprintln!("{} {} {} {:?}", count, delta, len, self);

        if count != delta {
            return Some(format!("Bitmaps mismatched, and not just off-by-one errors! count={} delta={}", count, delta));
        }


        if delta > allowed_off_by_one_bytes {
            return Some(format!("There were {} off-by-one errors, more than the {} ({}%) allowed.", delta, allowed_off_by_one_bytes, allowed_off_by_one_bytes as f64 / len as f64 * 100f64));
        }
        None
    }
}

#[derive(Clone)]
pub struct Constraints{
    pub similarity: Similarity,
    pub max_file_size: Option<usize>
}

pub enum ResultKind<'a>{
    Bitmap(&'a mut BitmapBgra),
    Bytes(&'a [u8])
}
impl<'a> ResultKind<'a>{
    fn exact_match_verbose(&mut self, c: &ChecksumCtx, name: &str) -> (ChecksumMatch, String){
        match *self{
            ResultKind::Bitmap(ref mut b) => c.bitmap_matches(*b, name),
            ResultKind::Bytes(ref b) => c.bytes_match(b, name)
        }
    }
}

fn get_imgref_bgra32(b: &mut BitmapBgra) -> imgref::ImgVec<rgb::RGBA<f32>> {
    use self::dssim::*;

    match b.fmt {
        s::PixelFormat::Bgra32 => {},
        s::PixelFormat::Bgr32 => {
            b.normalize_alpha().unwrap();
        },
        _ => unimplemented!(""),
    };


    assert_eq!(0, b.stride as usize % b.fmt.bytes());

    let stride_px = b.stride as usize / b.fmt.bytes();
    let pixels = unsafe {
        ::std::slice::from_raw_parts(b.pixels as *const rgb::alt::BGRA8, stride_px * b.h as usize + b.w as usize - stride_px)
    };

    assert!(pixels.len() >= b.w as usize * b.h as usize);

    imgref::Img::new_stride(pixels.to_rgbaplu(), b.w as usize, b.h as usize, stride_px)
}

/// Compare two bgra32 or bgr32 frames using the given similarity requirements
pub fn compare_bitmaps(_c: &ChecksumCtx,  actual: &mut BitmapBgra, expected: &mut BitmapBgra, require: Similarity, panic: bool) -> bool{
    let (count, delta) = diff_bitmap_bytes(actual, expected);

    if count == 0 {
        return true;
    }

    if let Similarity::AllowDssimMatch(minval, maxval) = require {
        let actual_ref = get_imgref_bgra32(actual);
        let expected_ref = get_imgref_bgra32(expected);
        let mut d = dssim::new();

        let actual_img = d.create_image(&actual_ref).unwrap();
        let expected_img = d.create_image(&expected_ref).unwrap();

        let (dssim, _) = d.compare(&expected_img, actual_img);

        let failure = if dssim > maxval {
           Some(format!("The dssim {} is greater than the permitted value {}", dssim, maxval))
        } else if dssim < minval {
            Some(format!("The dssim {} is lower than expected minimum value {}", dssim, minval))
        } else {
            None
        };

        if let Some(message) = failure {
            if panic {
                panic!("{}", message);
            } else {
                eprintln!("{}", message);
                false
            }
        } else {
            true
        }
    } else {
        if let Some(message) = require.report_on_bytes(count, delta, actual.w as usize * actual.h as usize * actual.fmt.bytes()){
            if panic{
                panic!("{}" ,message);
            }else{
                eprintln!("{}", message);
                return false;
            }
        }
        true
    }
}

/// Evaluates the given result against known truth, applying the given constraints
pub fn compare_with<'a, 'b>(c: &ChecksumCtx, expected_checksum: &str, expected_bitmap : &'b mut BitmapBgra, result: ResultKind<'a>, require: Constraints, panic: bool) -> bool{
    if !check_size(&result, require.clone(), panic) {
        return false;
    }


    let mut image_context = Context::create().unwrap();
    let actual_bitmap = match result {
        ResultKind::Bitmap(actual_bitmap) => actual_bitmap,
        ResultKind::Bytes(actual_bytes) => decode_input(&mut image_context, s::IoEnum::ByteArray(actual_bytes.to_vec()))
    };

    let result_checksum = ChecksumCtx::checksum_bitmap(actual_bitmap);




    if result_checksum == expected_checksum {
        true
    } else{
        compare_bitmaps(c, actual_bitmap, expected_bitmap, require.similarity, panic)
    }
}

pub fn check_size(result: &ResultKind, require: Constraints, panic: bool) -> bool{
    if let ResultKind::Bytes(ref actual_bytes) = *result {
        if actual_bytes.len() > require.max_file_size.unwrap_or(actual_bytes.len()) {
            let message = format!("Encoded size ({}) exceeds limit ({})", actual_bytes.len(), require.max_file_size.unwrap());
            if panic {
                panic!("{}", &message);
            } else {
                eprintln!("{}", &message);
                return false;
            }
        }

    }
    true
}




/// Evaluates the given result against known truth, applying the given constraints
pub fn evaluate_result<'a>(c: &ChecksumCtx, name: &str, mut result: ResultKind<'a>, require: Constraints, panic: bool) -> bool{
    let (exact, trusted) = result.exact_match_verbose(c, name);


    if !check_size(&result, require.clone(), panic) {
        return false;
    }


    if exact == ChecksumMatch::Match {
        true
    } else {
        let (expected_context, expected_bitmap) = c.load_image(&trusted);
        let mut image_context = Context::create().unwrap();
        let actual_bitmap = match result {
            ResultKind::Bitmap(actual_bitmap) => actual_bitmap,
            ResultKind::Bytes(actual_bytes) => {
                image_context.add_input_bytes(0, actual_bytes).unwrap();
                decode_image(&mut image_context, 0)
            }
        };

        let res = compare_bitmaps(c, actual_bitmap, unsafe{ &mut *expected_bitmap }, require.similarity, panic);
        drop(expected_context); // Context must remain in scope until we are done with expected_bitmap
        res
    }
}

/// Complains loudly and returns false  if `bitmap` doesn't match the stored checksum and isn't within the off-by-one grace window.
pub fn bitmap_regression_check(c: &ChecksumCtx, bitmap: &mut BitmapBgra, name: &str, allowed_off_by_one_bytes: usize) -> bool{

    evaluate_result(c, name, ResultKind::Bitmap(bitmap), Constraints{
        similarity: Similarity::AllowOffByOneBytesCount(allowed_off_by_one_bytes as i64),
        max_file_size: None
    }, true)
}




/// Compares the bitmap frame result of a given job to the known good checksum. If there is a checksum mismatch, a percentage of off-by-one bytes can be allowed.
/// If no good checksum has been stored, pass 'store_if_missing' in order to add it.
/// If you accidentally store a bad checksum, just delete it from the JSON file manually.
///
pub fn compare(input: Option<s::IoEnum>, allowed_off_by_one_bytes: usize, checksum_name: &str, store_if_missing: bool, debug: bool, steps: Vec<s::Node>) -> bool {

    compare_multiple(input.map(|i| vec![i]), allowed_off_by_one_bytes, checksum_name, store_if_missing, debug, steps)
}
pub fn compare_multiple(inputs: Option<Vec<s::IoEnum>>, allowed_off_by_one_bytes: usize, checksum_name: &str, store_if_missing: bool, debug: bool, steps: Vec<s::Node>) -> bool {
    let mut context = Context::create().unwrap();
    compare_with_context(&mut context, inputs, allowed_off_by_one_bytes, checksum_name, store_if_missing, debug, steps)
}

pub fn compare_with_context(context: &mut Context, inputs: Option<Vec<s::IoEnum>>, allowed_off_by_one_bytes: usize, checksum_name: &str, store_if_missing: bool, debug: bool, mut steps: Vec<s::Node>) -> bool {
    let mut bit = BitmapBgraContainer::empty();
    steps.push(unsafe{ bit.get_node()});

    //println!("{}", serde_json::to_string_pretty(&steps).unwrap());

    let build = s::Build001 {
        builder_config: Some(default_build_config(debug)),
        io: inputs.map(|v|
            v.into_iter()
                .enumerate()
                .map(|(ix,i)|i.into_input(ix as i32))
                .collect())
            .unwrap_or(Vec::new()),
        framewise: s::Framewise::Steps(steps)
    };

    if debug {
        println!("{}", serde_json::to_string_pretty(&build).unwrap());
    }


    let response = context.build_1(build).unwrap();

    if let Some(b) = unsafe { bit.bitmap(&context) } {
        if debug {
            println!("{:?}", b);
        }
        let mut ctx = ChecksumCtx::visuals(&context);
        ctx.create_if_missing = store_if_missing;
        bitmap_regression_check(&ctx, b, checksum_name, allowed_off_by_one_bytes)

    }else{
        panic!("execution failed {:?}", response);
    }
}


/// Compares the encoded result of a given job to the known good checksum. If there is a checksum mismatch, a percentage of off-by-one bytes can be allowed.
/// If no good checksum has been stored, pass 'store_if_missing' in order to add it.
/// If you accidentally store a bad checksum, just delete it from the JSON file manually.
///
/// The output io_id is 1
pub fn compare_encoded(input: Option<s::IoEnum>, checksum_name: &str, store_if_missing: bool, debug: bool, require: Constraints, steps: Vec<s::Node>) -> bool {
    let mut io = vec![s::IoEnum::OutputBuffer.into_output(1)];
    if let Some(i) = input{
        io.insert(0, i.into_input(0));
    }
    let build = s::Build001 {
        builder_config: Some(default_build_config(debug)),
        io,
        framewise: s::Framewise::Steps(steps)
    };

    if debug {
        println!("{}", serde_json::to_string_pretty(&build).unwrap());
    }

    let mut context = Context::create().unwrap();
    let _ = context.build_1(build).unwrap();

    let bytes = context.get_output_buffer_slice(1).unwrap();

    let mut ctx = ChecksumCtx::visuals(&context);
    ctx.create_if_missing = store_if_missing;



    evaluate_result(&ctx, checksum_name, ResultKind::Bytes(bytes), require, true)
}






/// Simplified graph recording configuration
pub fn default_build_config(debug: bool) -> s::Build001Config {
    s::Build001Config{
        graph_recording: if debug {Some(s::Build001GraphRecording::debug_defaults())} else {None},
    }
}

/// Simplifies access to raw bitmap data from Imageflow (when using imageflow_types::Node)
/// Consider this an unmovable type. If you move it, you will corrupt the heap.
pub struct BitmapBgraContainer{
    dest_bitmap: *mut imageflow_core::ffi::BitmapBgra
}
impl BitmapBgraContainer{
    pub fn empty() -> Self{
        BitmapBgraContainer{
            dest_bitmap: std::ptr::null_mut()
        }
    }
    /// Creates an operation node containing a pointer to self. Do not move self!
    pub unsafe fn get_node(&mut self) -> s::Node{
        let ptr_to_ptr = &mut self.dest_bitmap as *mut *mut imageflow_core::ffi::BitmapBgra;
        s::Node::FlowBitmapBgraPtr { ptr_to_flow_bitmap_bgra_ptr: ptr_to_ptr as usize}
    }

    /// Returns a reference the bitmap
    /// This reference is only valid for the duration of the context it was created within
    pub unsafe fn bitmap<'a>(&self, _: &'a Context) -> Option<&'a mut BitmapBgra>{
        if self.dest_bitmap.is_null(){
            None
        }else {
            Some(&mut *self.dest_bitmap)
        }
    }
}
