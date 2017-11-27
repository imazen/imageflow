use preludes::from_std::*;
use std;
use chrono::{DateTime};
use unicase::UniCase;
use errors::*;
use errors::Result;
use lockless::primitives::append_list::AppendList;
use lockless::primitives::append_list::AppendListIterator;
#[allow(unused)] use std::ascii::AsciiExt;
use chrono::Utc;
use std::thread;
use std::thread::JoinHandle;
use parking_lot::{Mutex, Condvar};
use std::panic::AssertUnwindSafe;
use std::any::Any;

// Get build date
// Get ticks
// Get utcnow
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

mod cache;
mod parsing;
mod compute;
mod support;
mod license_pair;

#[cfg(test)]
mod test;

use self::license_pair::*;
use self::support::*;
use self::cache::*;
use self::parsing::*;
use self::compute::*;
// IssueSink

pub trait LicenseClock: Sync + Send{
    fn get_timestamp_ticks(&self) -> u64;
    fn ticks_per_second(&self) -> u64;
    fn get_build_date(&self) -> DateTime<Utc>;
    fn get_utc_now(&self) -> DateTime<Utc>;
}


pub struct LicenseManagerSingleton{
    licenses: AppendList<License>,
    aliases_to_id: ::chashmap::CHashMap<Cow<'static, str>,Cow<'static, str>>,
    cached: ::chashmap::CHashMap<Cow<'static, str>,LicenseBlob>,
    #[allow(dead_code)]
    sink: IssueSink,
    trusted_keys: &'static [RSADecryptPublic],
    cache: Box<PersistentStringCache>,
    created: DateTime<Utc>,
    #[allow(dead_code)]
    uid: ::uuid::Uuid,
    heartbeat_count: AtomicU64,
    clock: Box<LicenseClock>,
    handle: Arc<::parking_lot::RwLock<Option<JoinHandle<()>>>>,
    licenses_fetched: Mutex<usize>,
    licenses_fetched_change: ::parking_lot::Condvar,
}

#[cfg(not(test))]
const URL: &'static str = "https://licenses-redirect.imazen.net";

#[cfg(test)]
const URL: &'static str = ::mockito::SERVER_URL;


impl LicenseManagerSingleton{
    pub fn new(trusted_keys: &'static [RSADecryptPublic], clock: Box<LicenseClock + Sync>, cache: Box<PersistentStringCache>) -> Self{
        let created = clock.get_utc_now();
        LicenseManagerSingleton{
            trusted_keys,
            clock,
            cache,
            cached: ::chashmap::CHashMap::new(),
            aliases_to_id: ::chashmap::CHashMap::new(),
            licenses: AppendList::new(),
            sink: IssueSink::new("LicenseManager"),
            created,
            uid: ::uuid::Uuid::new_v4(),
            heartbeat_count: ::std::sync::atomic::ATOMIC_U64_INIT,
            handle: Arc::new(::parking_lot::RwLock::new(None)),
            licenses_fetched: Mutex::new(0),
            licenses_fetched_change: Condvar::new(),
        }

    }
    #[cfg(test)]
    pub fn rewind_boot_time(mut self, seconds: i64) -> Self{
        self.created = self.created.checked_sub_signed(::chrono::Duration::seconds(seconds)).unwrap();
        self
    }
    #[cfg(test)]
    pub fn set_boot_time(mut self, time: DateTime<Utc>) -> Self{
        self.created = time;
        self
    }

    fn set_handle(&self, h: Option<JoinHandle<()>>){
        *self.handle.write() = h
    }


    pub fn create_thread(mgr: Arc<LicenseManagerSingleton>){
        let clone = mgr.clone();
        let handle = thread::spawn(move || {

            eprintln!("starting thread");
            let result = ::std::panic::catch_unwind( AssertUnwindSafe(move || {
                let _ = mgr.created();

                let client = ::reqwest::Client::new().unwrap();
                for license in mgr.iter_all() {
                    if let &License::Pair(ref p) = license {
                        let url = format!("{}/v1/licenses/latest/{}.txt", URL, p.secret());
                        eprintln!("requesting {}", &url);
                        let mut response = client.get(&url).send().unwrap();
                        if response.status().is_success() {
                            let mut buf = Vec::new();
                            response.read_to_end(&mut buf).unwrap();
                            let s = ::std::str::from_utf8(&buf).unwrap();
                            let blob = LicenseBlob::deserialize(mgr.trusted_keys, s, "remote license").unwrap();
                            p.update_remote(blob).unwrap();
                            eprintln!("Updating remote");
                            let mut fetched = mgr.licenses_fetched.lock();
                            *fetched = *fetched + 1;
                            mgr.licenses_fetched_change.notify_one();
                        }else{
                            eprintln!("{:?}", response);
                        }
                    }
                }
                ()
            }));
            if let Err(e) = result{

                struct PanicFormatter<'a>(pub &'a Any);
                impl<'a> std::fmt::Display for PanicFormatter<'a> {
                    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        if let Some(str) = self.0.downcast_ref::<String>() {
                            write!(f, "panicked: {}\n", str)?;
                        } else if let Some(str) = self.0.downcast_ref::<&str>() {
                            write!(f, "panicked: {}\n", str)?;
                        }
                        Ok(())
                    }
                }

                eprintln!("{}", PanicFormatter(&e));
            }
            eprintln!("finishing thread");
        });
        clone.set_handle(Some(handle));
    }

    pub fn wait_for(&self, fetch_count: usize){
        let mut fetched = self.licenses_fetched.lock();
        while *fetched < fetch_count{
            self.licenses_fetched_change.wait(&mut fetched);
        }
    }
    pub fn clock(&self) -> &LicenseClock{
        &*self.clock
    }
    pub fn created(&self) -> DateTime<Utc>{
        self.created
    }

    pub fn heartbeat(&self){
        let _ = self.heartbeat_count.fetch_add(1, Ordering::Relaxed);
        for _ in self.licenses.iter(){
            //trigger heartbeat
        }
    }

    pub fn get_by_id(&self, id: &str) -> Option<&License>{
        self.licenses.iter().find(|l| l.id().eq_ignore_ascii_case(id))
    }

    pub fn cached_remote(&self, id: &str) -> Option<::chashmap::ReadGuard<Cow<'static,str>,LicenseBlob>>{

        //
        self.cached.get(&Cow::Owned(id.to_owned()))

    }
    fn get_by_alias(&self, license: &Cow<'static, str>) -> Option<&License>{
        if let Some(id) = self.aliases_to_id.get(license){
            if let Some(lic) = self.get_by_id(&id){ //TODO: should be ID
                return Some(lic);
            }
        }
        None
    }

    pub fn add_static(&self, license: &'static str) -> Result<()>{
        self.get_or_add(&Cow::Borrowed(license)).map(|_| ())
    }

    pub fn get_or_add(&self, license: &Cow<'static, str>) -> Result<&License>{
        // License parsing involves several dozen allocations.
        // Not cheap; thus the aliases_to_id table and get_by_alias
        if let Some(lic) = self.get_by_alias(license){
            return Ok(lic)
        }

        let parsed = LicenseBlob::deserialize(self.trusted_keys, license.as_ref(), "local license")?;

        let id = parsed.fields().id().to_owned();

        self.aliases_to_id.insert(license.clone(), Cow::Owned(id.to_owned()));

        if let Some(lic) = self.get_by_id(&id){
            Ok(lic)
        }else{
            self.add_license(parsed)
        }
    }

    fn add_license(&self, parsed: LicenseBlob) -> Result<&License>{
        let id_copy = parsed.fields().id().to_owned();
        self.licenses.append(License::new(parsed)?);
        // This ensures that we never access duplicates (which can be created in race conditions)
        Ok(self.get_by_id(&id_copy).expect("License was just appended. Have atomics failed?"))
    }

    pub fn iter_all(&self) -> AppendListIterator<License>{
        self.licenses.iter()
    }

    pub fn iter_shared(&self) -> AppendListIterator<License>{
        self.licenses.iter()
    }

    pub fn compute_feature(&self, feature: &str) -> LicenseComputation{
        let required_features = ::smallvec::SmallVec::from_buf([feature]);
        LicenseComputation::new(self, None, LicenseScope::All, &required_features)
    }

    pub fn compute(&self, info: Option<&LicenseReportInfo>,
                   scope: LicenseScope, required_features: &::smallvec::SmallVec<[&str;1]>) -> LicenseComputation{
        LicenseComputation::new(self, info, scope, required_features)
    }

}

