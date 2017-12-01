
extern crate libc;
extern crate imageflow_types as s;
extern crate imageflow_helpers as hlp;
extern crate serde_json;
extern crate dssim;

extern crate itertools;
extern crate twox_hash;

use std::ffi::CString;
use std::path::Path;

use imageflow_core::{Context, ErrorKind, FlowError, CodeLocation};

use imageflow_core::ffi::BitmapBgra;
use std::collections::BTreeMap;
use std::fs::File;
use std::path::{PathBuf};
use std::io::Write;
use self::twox_hash::XxHash;
use std::hash::Hasher;
use std;
use imageflow_core;

use std::sync::RwLock;


// Encoder testing
// quantization - compare exactly.
// DSSIM compare
// Output size range acceptable.


/// Simplifies access to raw bitmap data from Imageflow (when using imageflow_types::Node)
pub struct BitmapBgraContainer{
    dest_bitmap: *mut imageflow_core::ffi::BitmapBgra
}
impl BitmapBgraContainer{
    pub fn empty() -> Self{
        BitmapBgraContainer{
            dest_bitmap: std::ptr::null_mut()
        }
    }
    pub unsafe fn get_node(&mut self) -> s::Node{
        let ptr_to_ptr = &mut self.dest_bitmap as *mut *mut imageflow_core::ffi::BitmapBgra;
        s::Node::FlowBitmapBgraPtr { ptr_to_flow_bitmap_bgra_ptr: ptr_to_ptr as usize}
    }
    pub unsafe fn ptr_to_bitmap(&self) -> *mut imageflow_core::ffi::BitmapBgra{
        self.dest_bitmap
    }

    pub unsafe fn bitmap(&self) -> Option<&mut BitmapBgra>{
        if self.dest_bitmap.is_null(){
            None
        }else {
            Some(&mut *self.dest_bitmap)
        }
    }
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
    max_off_by_one_ratio: f32,
    remote_storage_pattern: &'static str
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
            max_off_by_one_ratio: 0.01,
            remote_storage_pattern: "https://s3-us-west-2.amazonaws.com/imageflow-resources/visual_test_checksums/{}.png"
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
        /// Write the URL list
        /// We can use this to prefetch required images in the background on CI)
        /// TODO: add simple script to do this
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
        self.remote_storage_pattern.to_owned().replace("{}", checksum)
    }

    pub fn image_path(&self, checksum: &str) -> PathBuf{
        self.visuals_dir.as_path().join(Path::new(&format!("{}.png", &checksum)))
    }

    pub fn image_path_cstring(&self, checksum: &str) -> CString{
        CString::new(self.image_path(checksum).into_os_string().into_string().unwrap()).unwrap()
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
    /// YOU MUST CALL BitmapeBgra::destroy() to clean this up
    pub fn load_image(&self, checksum: &str) -> *mut BitmapBgra{
        self.fetch_image(checksum);
        unsafe {
            let cpath = self.image_path_cstring(checksum);
            let mut b: *mut BitmapBgra = std::ptr::null_mut();
            if !::imageflow_core::ffi::flow_bitmap_bgra_load_png(self.c.flow_c(), &mut b as *mut *mut BitmapBgra, cpath.as_ptr()) {
                cerror!(self.c).panic();
            }
            b
        }
    }

    /// Save the given image to disk by calculating its checksum.
    pub fn save_image(&self, bit: &BitmapBgra){
        let checksum =checksum_bitmap(bit);
        let dest_path = self.image_path(&checksum);
        if !dest_path.exists(){
            let dest_cpath = self.image_path_cstring(&checksum);
            println!("Writing {:?}", &dest_path);
            unsafe {
                if !::imageflow_core::ffi::flow_bitmap_bgra_save_png(self.c.flow_c(), bit as *const BitmapBgra, dest_cpath.as_ptr()){
                    cerror!(self.c).panic();
                }
            }
        }
    }
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
    if a.w != b.w || a.h != b.h || a.fmt.bytes() != b.fmt.bytes() { panic!("Bitmap dimensions differ. a:\n{:#?}\nb:\n{:#?}", a, b); }

    let width_bytes = a.w as usize * a.fmt.bytes();
    (0isize..a.h as isize).map(|h| {

        let a_contents_slice = unsafe { ::std::slice::from_raw_parts(a.pixels.offset(h * a.stride as isize), width_bytes) };
        let b_contents_slice = unsafe { ::std::slice::from_raw_parts(b.pixels.offset(h * b.stride as isize), width_bytes) };

        diff_bytes(a_contents_slice, b_contents_slice)

    }).fold((0, 0), |(a, b), (c, d)| (a + c, b + d))
}




pub fn checksum_bitmap(bitmap: &BitmapBgra) -> String {
    let info = format!("{}x{} fmt={}", bitmap.w, bitmap.h, bitmap.fmt as i32);
    return format!("{:02$X}_{:02$X}", bitmap.short_hash_pixels(), hlp::hashing::legacy_djb2(info.as_bytes()), 17)
}




pub fn regression_check(c: &ChecksumCtx, bitmap: &mut BitmapBgra, name: &str) -> bool{
    bitmap.normalize_alpha().unwrap();

    // Always write a copy if it doesn't exist
    c.save_image(bitmap);

    let trusted = c.get(name);
    let current = checksum_bitmap(bitmap);
    if trusted == None {
        if c.create_if_missing {
            println!("====================\n{}\nStoring checksum {}", name, &current);
            c.set(name.to_owned(), current.clone()).unwrap();
        } else {
            panic!("There is no stored checksum for {}; rerun with create_if_missing=true", name);
        }
        true
    }else if Some(&current) != trusted.as_ref() {
        println!("====================\n{}\nThe stored checksum {} differs from the current one {}", name, trusted.as_ref().unwrap(), &current);

        let trusted_bit = c.load_image(trusted.as_ref().unwrap());
        let (count, delta) = diff_bitmap_bytes(bitmap, unsafe{ &*trusted_bit});

        unsafe {
            BitmapBgra::destroy(trusted_bit, c.c);
        }
        if count != delta{
            panic!("Not just off-by-one errors! count={} delta={}", count, delta);
        }
        let allowed_errors = ((bitmap.w * bitmap.stride) as f32 * c.max_off_by_one_ratio) as i64;
        if delta  > allowed_errors{
            panic!("There were {} off-by-one errors, more than the {} ({}%) allowed.", delta, allowed_errors, c.max_off_by_one_ratio * 100f32);
        }
        true
        //Optionally run dssim/imagemagick
    }else{
        true //matched! yay!
    }
}





pub fn smoke_test(input: Option<s::IoEnum>, output: Option<s::IoEnum>,  debug: bool, steps: Vec<s::Node>){
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
    let _ = context.build_1(build).unwrap();

}

/// Compares the bitmap frame result of a given job to the known good checksum. If there is a checksum mismatch, a percentage of off-by-one bytes can be allowed.
/// If no good checksum has been stored, pass 'store_if_missing' in order to add it.
/// If you accidentally store a bad checksum, just delete it from the JSON file manually.
///
pub fn compare(input: Option<s::IoEnum>, allowed_off_by_one_bytes: usize, checksum_name: &str, store_if_missing: bool, debug: bool, mut steps: Vec<s::Node>) -> bool {
    let mut bit = BitmapBgraContainer::empty();
    steps.push(unsafe{ bit.get_node()});

    //println!("{}", serde_json::to_string_pretty(&steps).unwrap());

    let build = s::Build001 {
        builder_config: Some(default_build_config(debug)),
        io: input.map(|i| vec![i.into_input(0)]).unwrap_or(Vec::new()),
        framewise: s::Framewise::Steps(steps)
    };

    if debug {
        println!("{}", serde_json::to_string_pretty(&build).unwrap());
    }

    let mut context = Context::create().unwrap();
    let response = context.build_1(build).unwrap();

    if let Some(b) = unsafe { bit.bitmap() } {
        if debug {
            println!("{:?}", b);
        }
        let mut ctx = ChecksumCtx::visuals(&context);
        ctx.create_if_missing = store_if_missing;
        ctx.max_off_by_one_ratio = allowed_off_by_one_bytes as f32 / (b.h * b.stride) as f32;
        regression_check(&ctx, b, checksum_name)

    }else{
        panic!("execution failed {:?}", response);
    }
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

    if let Some(b) = unsafe { bit.bitmap() } {
        (b.w, b.h)
    }else{
        panic!("execution failed: {:?}", result);
    }
}






fn default_build_config(debug: bool) -> s::Build001Config {
    s::Build001Config{
        graph_recording: if debug {Some(s::Build001GraphRecording::debug_defaults())} else {None},
    }
}
