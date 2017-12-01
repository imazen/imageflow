
extern crate libc;
extern crate imageflow_types as s;
extern crate imageflow_helpers as hlp;
extern crate serde_json;
extern crate dssim;

extern crate twox_hash;

use std::ffi::CString;
use std::path::Path;

use imageflow_core::{Context, ErrorKind, FlowError, CodeLocation};

use imageflow_core::ffi::BitmapBgra;
use std::collections::HashMap;
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




fn default_build_config(debug: bool) -> s::Build001Config {
    s::Build001Config{
        graph_recording: if debug {Some(s::Build001GraphRecording::debug_defaults())} else {None},
    }
}

pub fn checksum_bitmap(bitmap: &BitmapBgra) -> String {
    unsafe {
        let info = format!("{}x{} fmt={}", bitmap.w, bitmap.h, bitmap.fmt as i32);
        let width_bytes = bitmap.w as usize *  bitmap.fmt.bytes();
        //TODO: Support Bgr32 properly by skipping alpha channel
        let mut hash = XxHash::with_seed(0x8ed1_2ad9_483d_28a0);
        for h in 0isize..(bitmap.h as isize){
            let row_slice = ::std::slice::from_raw_parts(bitmap.pixels.offset(h * bitmap.stride as isize), width_bytes);
            hash.write(row_slice)
        }
        return format!("{:02$X}_{:02$X}",hash.finish(), djb2(info.as_bytes()),17)
    }
}


fn djb2(bytes: &[u8]) -> u64{
    bytes.iter().fold(5381u64, |hash, c| ((hash << 5).wrapping_add(hash)).wrapping_add(u64::from(*c)))
}


pub struct ChecksumCtx<'a>{
    c: &'a Context,
    checksum_file: PathBuf,
    visuals_dir: PathBuf,
    #[allow(dead_code)]
    cache_dir: PathBuf,
    create_if_missing: bool,
    max_off_by_one_ratio: f32
}



lazy_static! {
    static ref CHECKSUM_FILE: RwLock<()> = RwLock::new(());
}

fn load_list(c: &ChecksumCtx) -> Result<HashMap<String,String>,()>{
    if c.checksum_file.exists() {
        let map: HashMap<String, String> = ::serde_json::from_reader(::std::fs::File::open(&c.checksum_file).unwrap()).unwrap();
        Ok(map)
    }else{
        Ok(HashMap::new())
    }
}
fn save_list(c: &ChecksumCtx, map: &HashMap<String,String>) -> Result<(),()>{
    let mut f = ::std::fs::File::create(&c.checksum_file).unwrap();
    ::serde_json::to_writer_pretty(&mut f, map).unwrap();

    f.sync_all().unwrap();
    Ok(())
}

#[allow(unused_variables)]
fn load_checksum(c: &ChecksumCtx, name: &str) -> Option<String>{
    #[allow(unused_variables)]
    let lock = CHECKSUM_FILE.read().unwrap();
    load_list(c).unwrap().get(name).and_then(|v|Some(v.to_owned()))
}
#[allow(unused_variables)]
fn save_checksum(c: &ChecksumCtx, name: String, checksum: String) -> Result<(),()>{
    #[allow(unused_variables)]
    let lock = CHECKSUM_FILE.write().unwrap();
    let mut map = load_list(c).unwrap();
    map.insert(name,checksum);
    save_list(c,&map).unwrap();
    Ok(())
}

fn fetch_bytes(url: &str) -> Vec<u8> {
    hlp::fetching::fetch_bytes(url).expect("Did you forget to upload {} to s3?")
}

fn download(c: &ChecksumCtx, checksum: &str){
    let dest_path = c.visuals_dir.as_path().join(Path::new(&format!("{}.png", checksum)));
    let source_url = format!("https://s3-us-west-2.amazonaws.com/imageflow-resources/visual_test_checksums/{}.png",checksum);
    if dest_path.exists() {
        println!("{} (trusted) exists", checksum);
    }else{
        println!("Fetching {} to {:?}", &source_url, &dest_path);
        File::create(&dest_path).unwrap().write_all(&fetch_bytes(&source_url)).unwrap();
    }
}
fn save_visual(c: &ChecksumCtx, bit: &BitmapBgra){
    let checksum =checksum_bitmap(bit);
    let dest_path = c.visuals_dir.as_path().join(Path::new(&format!("{}.png", &checksum)));
    if !dest_path.exists(){
        println!("Writing {:?}", &dest_path);
        let dest_cpath = CString::new(dest_path.into_os_string().into_string().unwrap()).unwrap();
        unsafe {
            if !::imageflow_core::ffi::flow_bitmap_bgra_save_png(c.c.flow_c(), bit as *const BitmapBgra, dest_cpath.as_ptr()){
                cerror!(c.c).panic();
            }
        }

    }
}

fn load_visual(c: &ChecksumCtx, checksum: &str) -> *const BitmapBgra{
    unsafe {
        let path = c.visuals_dir.as_path().join(Path::new(&format!("{}.png", &checksum)));
        let cpath = CString::new(path.into_os_string().into_string().unwrap()).unwrap();
        let mut b: *const BitmapBgra = std::ptr::null();
        if !::imageflow_core::ffi::flow_bitmap_bgra_load_png(c.c.flow_c(), &mut b as *mut *const BitmapBgra, cpath.as_ptr()) {
            cerror!(c.c).panic();
        }
        b
    }
}
/// Returns the number of bytes that differ, followed by the total value of all differences
/// If these are equal, then only off-by-one errors are occurring
fn diff_bytes(a: &[u8], b: &[u8]) ->(i64,i64){
    a.iter().zip(b.iter()).fold((0,0), |(count, delta), (a,b)| if a != b { (count + 1, delta + (i64::from(*a) - i64::from(*b)).abs()) } else { (count,delta)})
}


fn diff_bitmap_bytes(a: &BitmapBgra, b: &BitmapBgra) -> (i64,i64){
    if a.w != b.w || a.h != b.h || a.fmt.bytes() != b.fmt.bytes() { panic!("Bitmap dimensions differ. a:\n{:#?}\nb:\n{:#?}", a, b); }

    let width_bytes = a.w as usize * a.fmt.bytes();
    (0isize..a.h as isize).map(|h| {

        let a_contents_slice = unsafe { ::std::slice::from_raw_parts(a.pixels.offset(h * a.stride as isize), width_bytes) };
        let b_contents_slice = unsafe { ::std::slice::from_raw_parts(b.pixels.offset(h * b.stride as isize), width_bytes) };

        diff_bytes(a_contents_slice, b_contents_slice)

    }).fold((0, 0), |(a, b), (c, d)| (a + c, b + d))

}

pub fn regression_check(c: &ChecksumCtx, bitmap: *mut BitmapBgra, name: &str) -> bool{
    let bitmap_ref =unsafe{&mut *bitmap};
    bitmap_ref.normalize_alpha().unwrap();

    // Always write a copy if it doesn't exist
    save_visual(c, bitmap_ref);

    if bitmap.is_null(){panic!("");}
    let trusted = load_checksum(c, name);
    let current = checksum_bitmap(bitmap_ref);
    if trusted == None {
        if c.create_if_missing {
            println!("====================\n{}\nStoring checksum {}", name, &current);
            save_checksum(c, name.to_owned(), current.clone()).unwrap();
        } else {
            panic!("There is no stored checksum for {}; rerun with create_if_missing=true", name);
        }
        true
    }else if Some(&current) != trusted.as_ref() {
        download(c, trusted.as_ref().unwrap());
        println!("====================\n{}\nThe stored checksum {} differs from the current one {}", name, trusted.as_ref().unwrap(), &current);

        let trusted_bit = load_visual(c,trusted.as_ref().unwrap());
        let (count, delta) = diff_bitmap_bytes(bitmap_ref, unsafe{ &*trusted_bit});
        unsafe{
            ::imageflow_core::ffi::flow_destroy(c.c.flow_c(), trusted_bit as *const libc::c_void, std::ptr::null(), 0);
        }
        if count != delta{
            panic!("Not just off-by-one errors! count={} delta={}", count, delta);
        }
        let allowed_errors = ((bitmap_ref.w * bitmap_ref.stride) as f32 * c.max_off_by_one_ratio) as i64;
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

pub fn compare(input: Option<s::IoEnum>, allowed_off_by_one_bytes: usize, checksum_name: &str, store_if_missing: bool, debug: bool, mut steps: Vec<s::Node>) -> bool {
    let mut bit = BitmapBgraContainer::empty();

    let mut inputs = Vec::new();
    if input.is_some() {
        inputs.push(s::IoObject {
            io_id: 0,
            direction: s::IoDirection::In,
            io: input.unwrap()
        });
    }

    steps.push(unsafe{ bit.get_node()});

    //println!("{}", serde_json::to_string_pretty(&steps).unwrap());

    let build = s::Build001 {
        builder_config: Some(s::Build001Config {
            graph_recording: if debug {
                Some(s::Build001GraphRecording::debug_defaults())
            }else {
                None
            }
        }),
        io: inputs,
        framewise: s::Framewise::Steps(steps)
    };

    if debug {
        println!("{}", serde_json::to_string_pretty(&build).unwrap());
    }

    let mut context = Context::create().unwrap();
    let _ = context.build_1(build).unwrap();

    if let Some(b) = unsafe { bit.bitmap() } {
        if debug {
            println!("{:?}", b);
        }
        let mut ctx = checksums_ctx_for(&context);
        ctx.create_if_missing = store_if_missing;
        ctx.max_off_by_one_ratio = allowed_off_by_one_bytes as f32 / (b.h * b.stride) as f32;
        regression_check(&ctx, b as *mut BitmapBgra, checksum_name)

    }else{
        panic!("execution failed");
    }
}

pub fn checksums_ctx_for(c: &Context) -> ChecksumCtx{
    let visuals = Path::new(env!("CARGO_MANIFEST_DIR")).join(Path::new("tests")).join(Path::new("visuals"));
    std::fs::create_dir_all(&visuals).unwrap();
    ChecksumCtx {
        c,
        visuals_dir: visuals.clone(),
        cache_dir: visuals.join(Path::new("cache")),
        create_if_missing: true,
        checksum_file: visuals.join(Path::new("checksums.json")),
        max_off_by_one_ratio: 0.01
    }
}


pub fn get_result_dimensions(steps: &[s::Node], io: Vec<s::IoObject>, debug: bool) -> (u32, u32) {
    let mut steps = steps.to_vec();

    let mut bit = BitmapBgraContainer::empty();
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



