use crate::preludes::from_std::*;
use app_dirs::*;

use super::super::util::*;

const APP_INFO: AppInfo = AppInfo{name: "Imageflow", author: "Imazen"};


/// Currently implemented for a single folder
pub struct DiskStorage{
    filesystem: ::parking_lot::RwLock<bool>,
    folder: ::std::result::Result<PathBuf, AppDirsError>,
    data_kind: &'static str,
    log_stderr: bool,
    create_folder: bool
}

lazy_static!{
    static ref LICENSE_STORAGE: DiskStorage = DiskStorage::new("license", true, true);

    pub static ref LICENSE_CACHE: WriteThroughCache = WriteThroughCache::new("imageflow_license", &*LICENSE_STORAGE);
}

impl DiskStorage{

    fn new(data_kind: &'static str, log_stderr: bool, create_folder: bool) -> Self{
        DiskStorage{
            folder: app_dir(AppDataType::UserCache, &APP_INFO, "cache/licenses"),
            data_kind,
            log_stderr,
            create_folder,
            filesystem: ::parking_lot::RwLock::new(true)
        }
    }

    fn get_folder(&self) -> io::Result<&PathBuf>{
        match self.folder{
            Ok(ref b) => Ok(b),
            //            Err(AppDirsError::Io(ref e)) => Err(e.),
            Err(ref other) => Err(io::Error::new(io::ErrorKind::Other, format!("{:?}", other)))
        }
    }

    #[allow(dead_code)]
    fn try_delete<P>(&self, name: P)
                     -> io::Result<()> where P: AsRef<Path>, P: ::std::fmt::Debug {
        match self.try_delete_inner(name.as_ref()) {
            Err(e) => {
                if self.log_stderr{
                    eprintln!("Failed to delete {} named {:?} in folder {:?}: {:?}", self.data_kind, name.as_ref(), self.folder, e);
                }
                Err(e)
            },
            ok => ok
        }
    }


    #[allow(dead_code)]
    fn try_delete_inner<P>(&self, name: P)
                           -> io::Result<()> where P: AsRef<Path> {
        let _write_lock = self.filesystem.write();
        let path = self.get_folder()?.join(name);
        if path.is_file() {
            ::std::fs::remove_file(path)?;
        }
        Ok(())
    }

    fn try_write<P>(&self, name: P, value: &str)
                    -> io::Result<()> where P: AsRef<Path>, P: ::std::fmt::Debug {
        match self.try_write_inner(name.as_ref(), value) {
            Err(e) => {
                if self.log_stderr {
                    eprintln!("Failed to write {} named {:?} in folder {:?}: {:?}", self.data_kind, name.as_ref(), self.folder, e);
                }
                Err(e)
            },
            ok => ok
        }
    }
    fn try_write_inner<P>(&self, name: P, value: &str)
                          -> io::Result<()> where P: AsRef<Path>{
        let _write_lock = self.filesystem.write();
        if self.create_folder && !self.get_folder()?.is_dir(){
            ::std::fs::create_dir_all(self.get_folder()?)?;

        }
        let path = self.get_folder()?.join(name);
        let mut file = File::create(path)?;
        file.write_all(value.as_bytes())?;
        Ok(())
    }



    fn try_read<P>(&self, name: P)
                   -> io::Result<Option<String>> where P: AsRef<Path>, P: ::std::fmt::Debug {
        match self.try_read_inner(name.as_ref()) {
            Err(e) => {
                if self.log_stderr {
                    eprintln!("Failed to read {} named {:?} in folder {:?}: {:?}", self.data_kind, name.as_ref(), self.folder, e);
                }
                Err(e)
            },
            Ok(Some(v)) => {
                match String::from_utf8(v){
                    Err(e) => {
                        if self.log_stderr {
                            eprintln!("Invalid UTF8-bytes in {} named {:?} in folder {:?}: {:?}", self.data_kind, name.as_ref(), self.folder, e);
                        }
                        Err(io::Error::new(io::ErrorKind::InvalidData, e))
                    },
                    Ok(s) => Ok(Some(s))
                }
            }
            Ok(None) => Ok(None)
        }
    }

    fn try_read_inner<P>(&self, name: P)
                         -> io::Result<Option<Vec<u8>>> where P: AsRef<Path> {
        let _read_lock = self.filesystem.read();
        let path = self.get_folder()?.join(name);
        if path.is_file() {
            let mut file = File::open(path)?;
            let mut vec = Vec::new();
            let _ = file.read_to_end(&mut vec)?;
            Ok(Some(vec))
        }else {
            Ok(None)
        }
    }
}

// expired
// issued in future
// covers older builds
// revoked
// doesn't cover imageflow

pub enum StringCachePutResult {
    Duplicate,
    WriteComplete,
    WriteFailed
}
pub trait PersistentStringCache: Sync + Send{

    fn try_put(&self, key: &String, value: &str) -> StringCachePutResult;
    fn get(&self, key: &String) -> Option<String>;
}

///
/// Provides a mechanism to cache retrieved files
pub struct WriteThroughCache{
    prefix: &'static str,
    // Would be nice if we could lookup by &str instead of &String
    cache: ::chashmap::CHashMap<String, String>,
    disk: &'static DiskStorage
}


impl WriteThroughCache{
    pub fn new(prefix: &'static str, disk: &'static DiskStorage) -> Self{
        WriteThroughCache{
            prefix,
            cache: ::chashmap::CHashMap::new(),
            disk
        }
    }
    fn hash_to_base16(data: &str) -> String{
        let hash = crate::hashing::hash_256(data.as_bytes());
        crate::hashing::bytes_to_hex(&hash)
    }

    fn filename_key_for(&self, key: &str) -> String{
        if key.len() + self.prefix.len() > 200 || key.chars().any(|c| !c.is_alphanumeric() && c != '_' ){
            format!("{}{}.txt",self.prefix, Self::hash_to_base16(key))
        }else{
            format!("{}{}.txt", self.prefix, key)
        }
    }
    #[allow(dead_code)]
    pub fn as_cache(&self) -> &dyn PersistentStringCache{
        self
    }
}

impl PersistentStringCache for WriteThroughCache{
    fn try_put(&self, key: &String, value: &str) -> StringCachePutResult {
        if let Some(v) = self.cache.get(key){
            if *v == value{
                return StringCachePutResult::Duplicate;
            }
        }
        let _ = self.cache.insert(key.to_owned(), value.to_owned());
        if self.disk.try_write(self.filename_key_for(&key), &value).is_ok(){
            StringCachePutResult::WriteComplete
        }else{
            StringCachePutResult::WriteFailed
        }
    }

    fn get(&self, key: &String) -> Option<String> {
        if let Some(value) = self.cache.get(key){
            Some(value.to_owned())
        }else{
            if let Ok(Some(disk_value)) = self.disk.try_read(self.filename_key_for(&key)){
                let _ = self.cache.insert(key.to_owned(), disk_value.to_owned());
                Some(disk_value)
            }else {
                None
            }
        }
    }
}
