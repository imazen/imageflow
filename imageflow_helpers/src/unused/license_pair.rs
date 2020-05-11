use super::*;
use smallvec::SmallVec;
//lazy_static! {
//    static ref DEFAULT_LICENSE_SERVERS: Vec<&'static str> = vec!["https://s3-us-west-2.amazonaws.com/licenses.imazen.net/", "https://licenses-redirect.imazen.net/", "https://licenses.imazen.net", "https://licenses2.imazen.net"];
//}
use super::super::util::*;

pub enum License{
    Pair(LicensePair),
    Single(LicenseBlob)
}
impl License{
    pub fn id(&self) -> &str{
        match *self{
            License::Single(ref b) => b.fields().id(),
            License::Pair(ref p) => p.id()
        }
    }
    pub fn new(parsed: LicenseBlob) -> Result<License>{
        if parsed.fields().is_remote_placeholder(){
            Ok(License::Pair(LicensePair::new(parsed)?))
        }else{
            Ok(License::Single(parsed))
        }
    }
    pub fn is_pair(&self) -> bool{
        if let License::Pair(_) = *self{
            true
        }else {
            false
        }
    }
    pub fn remote_fetched(&self) -> bool{
        if let License::Pair(ref p) = *self{
            p.remote_fetched()
        }else {
            false
        }
    }

    pub fn first(&self) -> &LicenseBlob{
        match *self {
            License::Single(ref b) => b,
            License::Pair(ref p) => &p.placeholder()
        }
    }

    pub fn fresh_remote(&self) -> Option<::parking_lot::RwLockReadGuard<Option<LicenseBlob>>>{
        match *self {
            License::Single(..) => None,
            License::Pair(ref p) => Some(p.fresh_remote())
        }
    }

    pub fn dates(&self, manager: &LicenseManagerSingleton) -> SmallVec<[DateTime<Utc>;4]>{
        let mut vec = SmallVec::new();

        if let Some(d) = self.first().fields().issued(){
            vec.push(d);
        }
        if let Some(d) = self.first().fields().expires(){
            vec.push(d);
        }
        if let Some(read) = self.fresh_remote(){
            if let Some(ref license) = *read{
                if let Some(d) = license.fields().issued(){
                    vec.push(d);
                }
                if let Some(d) = license.fields().expires(){
                    vec.push(d);
                }
            }
        }else if let Some(license) = manager.cached_remote(self.id()){
            if let Some(d) = license.fields().issued(){
                vec.push(d);
            }
            if let Some(d) = license.fields().expires(){
                vec.push(d);
            }
        }
        vec
    }
}


pub struct LicensePair{
    id: String,
    secret: String,
    #[allow(dead_code)]
    cache_key: String,
    placeholder: LicenseBlob,
    #[allow(dead_code)]
    license_server_stack: Vec<Cow<'static,str>>,
    remote: Arc<::parking_lot::RwLock<Option<LicenseBlob>>>,
}
impl LicensePair{
    pub fn new(placeholder: LicenseBlob) -> Result<Self>{
        let id =  placeholder.fields().id().to_owned();
        let secret = placeholder.fields().secret().ok_or_else(|| "Remote placeholder license does not contain required field 'secret'.")?.to_owned();
        let cache_key = format!("{}_{:x}", &id, crate::hashing::hash_64(secret.as_bytes()));
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

    pub fn remote_fetched(&self) -> bool{
        self.remote.read().is_some()
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
