use ::preludes::from_std::*;
use ::std;
use num::{One, Zero};
use num::bigint::{BigInt, Sign};
use sha2::{Sha512, Digest};
use ::chrono::{DateTime,FixedOffset};
use unicase::UniCase;
use ::app_dirs::*;
use errors::*;
use errors::Result;
use ::lockless::primitives::append_list::AppendList;
use lockless::primitives::append_list::AppendListIterator;
use std::ascii::AsciiExt;
use chrono::Utc;

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

use self::license_pair::*;
use self::support::*;
use self::cache::*;
use self::parsing::*;
use self::compute::*;
// IssueSink

pub trait LicenseClock{
    fn get_timestamp_ticks(&self) -> u64;
    fn ticks_per_second(&self) -> u64;
    fn get_build_date(&self) -> DateTime<FixedOffset>;
    fn get_utc_now(&self) -> DateTime<Utc>;
}


pub struct LicenseManagerSingleton{
    licenses: AppendList<License>,
    aliases_to_id: ::chashmap::CHashMap<Cow<'static, str>,Cow<'static, str>>,
    sink: IssueSink,
    clock: &'static LicenseClock,
    trusted_keys: &'static [RSADecryptPublic],
    cache: &'static PersistentStringCache,
    created: DateTime<Utc>,
    uid: ::uuid::Uuid,
    heartbeat_count: AtomicU64,

}
impl LicenseManagerSingleton{
    pub fn new(trusted_keys: &'static [RSADecryptPublic], clock: &'static LicenseClock, cache: &'static PersistentStringCache) -> Self{
        LicenseManagerSingleton{
            trusted_keys,
            clock,
            cache,
            aliases_to_id: ::chashmap::CHashMap::new(),
            licenses: AppendList::new(),
            sink: IssueSink::new("LicenseManager"),
            created: ::chrono::Utc::now(),
            uid: ::uuid::Uuid::new_v4(),
            heartbeat_count: ::std::sync::atomic::ATOMIC_U64_INIT,
        }
    }

    pub fn created(&self) -> DateTime<Utc>{
        self.created
    }

    pub fn heartbeat(&self){
        let _ = self.heartbeat_count.fetch_add(1, Ordering::Relaxed);
        for l in self.licenses.iter(){
            //trigger heartbeat
        }
    }

    pub fn get_by_id(&self, id: &str) -> Option<&License>{
        self.licenses.iter().find(|l| l.id().eq_ignore_ascii_case(id))
    }
    fn get_by_alias(&self, license: &Cow<'static, str>) -> Option<&License>{
        if let Some(id) = self.aliases_to_id.get(license){
            if let Some(lic) = self.get_by_id(license.as_ref()){
                return Some(lic);
            }
        }
        None
    }

    pub fn get_or_add(&self, license: &Cow<'static, str>) -> Result<&License>{
        if let Some(lic) = self.get_by_alias(license){
            return Ok(lic)
        }

        // License parsing involves several dozen allocations.
        // Not cheap; thus the aliases_to_id table
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
        self.licenses.append(License::new(parsed, self.cache)?);
        // This ensures that we never access duplicates (which can be created in race conditions)
        Ok(self.get_by_id(&id_copy).expect("License was just appended. Have atomics failed?"))
    }

    pub fn iter_all(&self) -> AppendListIterator<License>{
        self.licenses.iter()
    }
    pub fn iter_shared(&self) -> AppendListIterator<License>{
        self.licenses.iter()
    }


}



struct LicenseFetcher;


