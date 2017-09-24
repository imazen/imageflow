use super::*;

use ::smallvec::SmallVec;
use std::iter::FromIterator;
use chrono::Offset;

lazy_static! {
    static ref DEFAULT_LICENSE_SERVERS: Vec<&'static str> = vec!["https://s3-us-west-2.amazonaws.com/licenses.imazen.net/", "https://licenses-redirect.imazen.net/", "https://licenses.imazen.net", "https://licenses2.imazen.net"];
}

pub struct LicensePair{
    id: String,
    secret: String,
    cache_key: String,
    placeholder: LicenseBlob,
    license_server_stack: Vec<Cow<'static,str>>,
    remote: Arc<::parking_lot::RwLock<Option<LicenseBlob>>>,
}
impl LicensePair{
    pub fn new(placeholder: LicenseBlob) -> Result<Self>{
        let id =  placeholder.fields().id().to_owned();
        let secret = placeholder.fields().secret().ok_or_else(|| "Remote placeholder license does not contain required field 'secret'.")?.to_owned();
        let cache_key = format!("{}_{:x}", &id, ::hashing::hash_64(secret.as_bytes()));
        Ok(LicensePair{
            id,
            secret,
            license_server_stack: Vec::new(),
            remote: Arc::new(::parking_lot::RwLock::new(None)),
            placeholder,
            cache_key
        })
    }
    pub fn id(&self) -> &str{
        &self.id
    }
    pub fn secret(&self) -> &str{
        &self.secret
    }
    pub fn update_remote(&self, remote: LicenseBlob) -> Result<()>{
        if !self.id().eq_ignore_ascii_case(remote.fields().id()){
            return Err(Error::from_kind(ErrorKind::LicenseCorrupted(format!("Remote license file does not match. Please contact support@imazen.io. Local id: {}, Remote id: {}",self.id(), remote.fields().id()))));
        }
        *self.remote.write() = Some(remote);
        Ok(())
    }

    pub fn is_pending(&self) -> bool{
        self.remote.read().is_none()
    }

    pub fn placeholder(&self) -> &LicenseBlob{
        &self.placeholder
    }

    pub fn fresh_remote(&self) -> ::parking_lot::RwLockReadGuard<Option<LicenseBlob>>{
        self.remote.read()
    }

    // Caching with write-through
    // Deserialization - error sink, trusted keys needed
    // Update License Servers

//
//    fn get_cached(&self) -> Option<LicenseBlob>{
//        if let Some(s) = self.cache.get(&self.cache_key){
//            if !s.is_ascii_digit(){
//                LicenseBlob::deserialize()
//            }
//        }
//    }
}
