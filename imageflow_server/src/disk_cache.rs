/// This is a naive local 'caching' implementation of a key/value blob store
/// Each pair gets 1 file
/// Hash collisions are improbable - we use blake2 256, faster than SHA-3. 32-byte hashes
/// Write only
/// Append-only log for transitioning to more complex system
/// Staging folder - files are renamed into final locations
/// soft and hard count and byte limit - NO DELETION
use std::path::*;
use std::io;
use std;
use std::io::prelude::*;
use std::fs::{create_dir_all, File};
use std::sync::atomic::{AtomicBool, Ordering};
use self::rand::RngCore;
// TODO:
// Cleanup staging folders automatically (failed renames)
// Implement write-only log
// Implement transactional filesystem 'counters' to track total count/size
// Implement write failure when limits are reached

// It *is* possible to implement FIFO cache eviction, but is FIFO worth it? (random sampling of the write log, staging folders to drop handles, etc)

extern crate rand;
extern crate imageflow_helpers;
use self::imageflow_helpers as hlp;


fn create_dir_all_helpful<P: AsRef<Path>>(path: P) -> io::Result<()> {
    match create_dir_all(&path) {
        Ok(v) => Ok(v),
        Err(e) => {
            //panic!("Failed to create dir {:?} {:?}", path.as_ref(), e);
            Err(e)
        }
    }
}


//
///// Cache of the state of the cache folder - can be invalidated by failure.
//struct CacheCache{
//
//}

//Since we have a fixed number of folders, known at creation time, let's deterministically order them
//and use a bitset or something.
//We can use RwLock on a BitVec or a Vec of AtomicBools (64kb vs 8kb, but maybe we just collapse for storage?)

#[derive(Debug)]
pub struct CacheFolder{
    root: PathBuf,
    root_confirmed: AtomicBool,
    meta_dir: PathBuf,
    staging_dir: PathBuf,
    meta_layout_confirmed: AtomicBool,
    write_log: PathBuf,
    consumption_log: PathBuf,
    consumption_summary: PathBuf,
    folder_bits: u8,
    folders_from_hash: u32,
    bits_format: &'static str,
    write_layout: FolderLayout
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FolderLayout {
    /// 64 tier 1 folders, each with a 'files' subdirectory. Optimal range 0 to 8,000 entries or so. Suggested max 51k.
    Tiny,
    /// 64 x 64, each leaf with a 'files' subdirectory. Optimal range ~8,000 to ~500,000 entries. Suggested max 3 million.
    Normal,
    /// 64 x 64 x 16. Optimal for 500k to 8 million entries. Suggested max 50 million.
    Huge
}

impl CacheFolder{
    pub fn new(root: &Path, write_layout: FolderLayout) -> CacheFolder{
        CacheFolder{
            meta_layout_confirmed: AtomicBool::default(),
            root: root.to_owned(),
            root_confirmed: AtomicBool::default(),
            meta_dir: root.join(Path::new("meta")),
            staging_dir: root.join(Path::new("staging")),
            write_log: root.join(Path::new("meta")).join(Path::new("write_log")),
            consumption_log: root.join(Path::new("meta")).join(Path::new("consumption_log")),
            consumption_summary: root.join(Path::new("meta")).join(Path::new("consumption_summary")),
            folder_bits: match write_layout{
                FolderLayout::Tiny => 6,
                FolderLayout::Normal => 12,
                FolderLayout::Huge => 16,
            },
            bits_format: match write_layout{
                FolderLayout::Tiny => "{250-256:02x}/files/{0-256:064x}",
                FolderLayout::Normal => "{250-256:02x}/{244-250:02x}/files/{0-256:064x}",
                FolderLayout::Huge => "{250-256:02x}/{244-250:02x}/{240-244:02x}/{0-256:064x}",
            },
            folders_from_hash: match write_layout{
              FolderLayout::Tiny => 64 * 2,
                FolderLayout::Normal => 64 * 64 * 2 + 64,
                FolderLayout::Huge => 64 * 64 * 16 + 64 * 64 + 64,
            },
            write_layout: write_layout
        }
    }

    pub fn entry(&self, hash: &[u8;32]) -> CacheEntry {
        CacheEntry {
            path: self.root.join(hlp::hashing::normalize_slashes(hlp::hashing::bits_format(hash, self.bits_format))),
            hash: *hash,
            parent: self
        }
    }
// PUT A README IN THE CACHE ROOT! DO IT!


    fn ensure_root(&self) -> io::Result<()>{
        if !self.root_confirmed.load(Ordering::Relaxed) &&
            !self.root.as_path().is_dir(){
            create_dir_all_helpful(&self.root)?;
            self.root_confirmed.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    fn ensure_meta_layout_confirmed(&self) -> io::Result<()>{
        if !self.meta_layout_confirmed.load(Ordering::SeqCst){
            let path = self.meta_dir.join(Path::new(match self.write_layout{
                FolderLayout::Huge => "huge",
                FolderLayout::Tiny => "tiny",
                FolderLayout::Normal => "normal"
            }));
            if !self.meta_layout_confirmed.load(Ordering::SeqCst) && !path.exists() {
                create_dir_all_helpful(&self.meta_dir)?;
                File::create(path)?;
                self.meta_layout_confirmed.store(true, Ordering::SeqCst);
            }
        }
        Ok(())
    }

    ///TODO: we could optimize directory existence checks with an 8, 512, or 8kb BitVec, easily persisted.
    /// We would need 'fast path' that falls back to 'careful path' when any of those caches get out of sync
    fn prepare_for(&self, entry: &CacheEntry) -> io::Result<()> {
        self.ensure_root().unwrap();
        self.ensure_meta_layout_confirmed().unwrap();
        let dir = entry.path.as_path().parent().expect("Every cache path should have a parent dir; this did not!");
        if !dir.exists(){
            create_dir_all_helpful(dir)?;
        }
        Ok(())
    }

    fn acquire_staging_location(&self, hash: &[u8;32]) -> io::Result<PathBuf>{
        if !self.staging_dir.as_path().exists(){
            create_dir_all_helpful(self.staging_dir.as_path())?;
        }

        let slot_id = hlp::timeywimey::time_bucket(60 * 60 * 2, 6);

        //six slots, each used for 2 hours.
        let subdir = self.staging_dir.join(Path::new(&format!("{}", slot_id)));
        if !subdir.exists() {
            create_dir_all_helpful(subdir.as_path())?;
        }

        let staging_path = format!("{:064x}_{:016x}_incoming", hlp::hashing::HexableBytes(hash), rand::thread_rng().next_u64());
        Ok(subdir.join(Path::new(&staging_path)))
    }
}

pub struct CacheEntry<'a>{
    //Path shall always have a valid parent.
    hash: [u8;32],
    path: PathBuf,
    parent: &'a CacheFolder
}

impl<'a> CacheEntry<'a>{
    pub fn prepare_dir(&self) -> io::Result<()>{
        self.parent.prepare_for(self)
    }
    pub fn exists(&self) -> bool{
        self.path.as_path().is_file()
    }

   // static NEXT_FLUENT_NODE_ID: AtomicU64 = ATOMIC_U64_INIT;


    // We have to write to a different file, first. Then we fs::rename() to overwrite
    //
    pub fn write(&self, bytes: &[u8]) -> io::Result<()> {
        self.prepare_dir()?;
        let temp_path = self.parent.acquire_staging_location(&self.hash)?;
        use ::std::fs::OpenOptions;
        {
            let mut f = OpenOptions::new().write(true).create_new(true).open(&temp_path)?;
            f.write_all(bytes)?;
        }
        std::fs::rename(&temp_path, &self.path)?;
        Ok(())
    }

    pub fn read(&self) -> io::Result<Vec<u8>>{
        hlp::filesystem::read_file_bytes(&self.path)
    }


}

// If one migrates from one FolderLayout to another, or is moving off of a old cache directory, then multiple queries make sense
// Check for meta/tiny, meta/normal, meta/huge presence to auto-populate
//struct CacheReader{
//    folders: Vec<CacheFolder>
//}
//
//impl CacheReader{
//
//}
//
//
//
























//We should log each 32-byte hash saved to a file. This would be far faster than a directory listing later.
//128 entries per block is quite efficient. We could handle 10 million in 320mb.

//But if we don't require aligning to block boundaries, another 11 to 16 bytes would be quite handy.

//We could log size for another 8 bytes (or 4 or 5 if we are ok with a 2 or 512GB limit)
//Dimensions are u16xu16 for the formats we care about. at least 1 byte to cover all mime types
//And another 2-3 bytes for useful metadata about the image??
// I.e, 43-48 bytes per record.

//We can append to file pretty reliably, but reads will get torn at the end https://stackoverflow.com/questions/1154446/is-file-append-atomic-in-unix






//
//hashes are 32 bytes?
//
//Cache write
//Create 3 parent directories
//
//
//
//
//There is no cleanup
//64 x 64 x 1 nested directories by default. Optional 64x64x16 + 1. with read through
//
//
//
//
//
//
//Allow read-through to
//There is no index
//
//
//Fetch by hash - blake2d 256bit
//
//blake2-rfc = "0.2.17" or https://github.com/RustCrypto/hashes
//base 32?
//
//
//Filesystem
//Disable 8.3 on windows
//Disable all metadata (as much as possible)
//only list unsorted
//https://stackoverflow.com/questions/197162/ntfs-performance-and-large-volumes-of-files-and-directories








