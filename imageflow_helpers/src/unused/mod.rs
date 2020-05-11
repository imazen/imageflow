#![allow(bad_style)]
#![allow(unused)]
use crate::preludes::from_std::*;
use std;
use chrono::{DateTime};
use unicase::UniCase;
use crate::errors::*;
use crate::errors::Result;
use lockless::primitives::append_list::AppendList;
use lockless::primitives::append_list::AppendListIterator;
use chrono::Utc;
use std::thread;
use std::thread::JoinHandle;
use parking_lot::{Mutex, Condvar, RwLock};
use std::panic::AssertUnwindSafe;
use std::any::Any;
use ::smallvec::SmallVec;
use std::iter::FromIterator;
use super::util::*;
// Get build date
// Get ticks
// Get utcnow
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

mod cache;
mod parsing;
mod compute;
pub mod support;
mod license_pair;

#[cfg(test)]
mod test;

use self::license_pair::*;
use self::support::*;
use self::cache::*;
use self::parsing::*;
use self::compute::*;
use super::pollster::*;
// IssueSink



pub struct LicenseEndpoint{
    path: String,
    id: String,
    mgr: Arc<LicenseManagerSingleton>,
    base_urls: RwLock<SmallVec<[Cow<'static, str>;6]>>
}

impl LicenseEndpoint{
    pub fn new(mgr: Arc<LicenseManagerSingleton>, license: &License) -> Self{
        LicenseEndpoint{
            path: format!("/v1/licenses/latest/{}.txt", license.first().fields().secret().expect("secret is required")),
            mgr,
            base_urls: RwLock::new(Self::default_urls()),
            id: license.id().to_owned()
        }
    }

    fn default_urls() -> SmallVec<[Cow<'static, str>;6]> {
        SmallVec::from_iter([
        Cow::Borrowed("https://s3-us-west-2.amazonaws.com/licenses.imazen.net/"),
        Cow::Borrowed("https://licenses-redirect.imazen.net/"),
        Cow::Borrowed("https://licenses.imazen.net/"),
        Cow::Borrowed("https://licenses2.imazen.net")].iter().map(|v| v.clone()))
    }
    pub fn box_endpoint(self) -> Box<dyn Endpoint>{
        Box::new(self)
    }
}

impl Endpoint for LicenseEndpoint{
    fn redact(&self, str: &mut str) {
        //TODO: implement secret redaction
    }

    fn get_fetch_interval(&self) -> ::chrono::Duration {
        ::chrono::Duration::hours(1)
    }

    fn get_query(&self) -> ::std::result::Result<&str, String> {
        Ok("")
    }

    fn get_path(&self) -> ::std::result::Result<&str, String> {
        Ok(&self.path)
    }

    fn get_base_urls(&self) -> SmallVec<[Cow<'static, str>; 6]> {
        SmallVec::from_iter(self.base_urls.read().iter().map(|c| c.clone()))
    }

    fn process_response(&self, content_type: Option<&::reqwest::header::HeaderValue>, bytes: Vec<u8>) -> ::std::result::Result<(), String> {
        let s =  ::std::str::from_utf8(&bytes).unwrap();
        let blob = LicenseBlob::deserialize(self.mgr.trusted_keys, s, "remote license").unwrap();
        if let Some(&License::Pair(ref p)) = self.mgr.get_by_id(&self.id){
            p.update_remote(blob.clone()).map_err(|e| format!("{}", e))?;
        }
        // TODO: update base urls
        self.mgr.update_cache(&self.id, blob);
        Ok(())
    }
}


pub struct LicenseManagerSingleton{
    licenses: AppendList<License>,
    aliases_to_id: ::chashmap::CHashMap<Cow<'static, str>,String>,
    cached: ::chashmap::CHashMap<Cow<'static, str>,LicenseBlob>,
    #[allow(dead_code)]
    sink: IssueSink,
    trusted_keys: &'static [RSADecryptPublic],
    cache: Box<dyn PersistentStringCache>,
    created: DateTime<Utc>,
    #[allow(dead_code)]
    uid: ::uuid::Uuid,
    heartbeat_count: AtomicUsize,
    clock: Arc<dyn AppClock>,
    fetcher_token: Arc<SharedToken>
}




#[cfg(not(test))]
fn GetServerUrl() -> String{
    "https://licenses-redirect.imazen.net".to_owned()
}
#[cfg(test)]
fn GetServerUrl() -> String{
    ::mockito::server_url()
}


impl LicenseManagerSingleton{
    pub fn new(trusted_keys: &'static [RSADecryptPublic], clock: Arc<dyn AppClock>, cache: Box<dyn PersistentStringCache>) -> Self{
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
            heartbeat_count: ::std::sync::atomic::AtomicUsize::new(0),
            fetcher_token: Arc::new(SharedToken::new())
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

    pub fn create_thread(mgr: Arc<LicenseManagerSingleton>){
        let mgr_clone = mgr.clone();
        Fetcher::ensure_spawned(mgr.fetcher_token.clone(), mgr.clock.clone(),
                                move || mgr_clone.licenses.iter()
            .filter(|lic| lic.is_pair())
            .map(|lic| LicenseEndpoint::new(mgr_clone.clone(), lic).box_endpoint())
            .collect());
    }

    pub fn fetcher_token(&self) -> Arc<SharedToken>{
        self.fetcher_token.clone()
    }


    pub fn begin_kill_thread(&self, timeout_ms: u64) {
        if !self.fetcher_token().wait_for_shutdown(::std::time::Duration::from_millis(timeout_ms)) {
            panic!("Failed to shutdown fetcher thread within {}ms", timeout_ms);
        }
    }

    pub fn kill_thread(arc_mgr: Arc<LicenseManagerSingleton>, timeout_ms: u64) {
        let start = crate::timeywimey::precise_time_ns();
        let mut m = arc_mgr;
        m.begin_kill_thread(timeout_ms);
        loop {
            match Arc::try_unwrap(m) {
                Ok(mgr) => {
                    mgr.join_thread();
                    return;
                }
                Err(arc_mgr) => {
                    if (crate::timeywimey::precise_time_ns() - start) / 1000 > timeout_ms {
                        panic!("Failed to join fetcher thread within {}ms", timeout_ms);
                    } else {
                        thread::sleep(::std::time::Duration::from_millis(15));
                        m = arc_mgr;
                    }
                }
            }
        }
    }


    pub fn join_thread(self) {
        match Arc::try_unwrap(self.fetcher_token){
            Ok(t) => {
                t.join_thread().unwrap();
            },
            Err(arc) => {
                panic!("Other references to the fetcher token exist. They must be dropped before the thread can be killed");
            }
        }
    }


    pub fn clock(&self) -> &dyn AppClock {
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
    pub fn update_cache(&self, id: &str, license: LicenseBlob){
        self.cached.insert(Cow::Owned(id.to_owned()), license);

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

        self.aliases_to_id.insert(license.clone(), id.to_owned());

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

