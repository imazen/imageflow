use super::*;

use ::smallvec::SmallVec;
use std::iter::FromIterator;

pub enum License{
    Pair(LicensePair),
    Single(LicenseBlob)
}
impl License{
    pub fn id(&self) -> &str{
        match self{
            &License::Single(ref b) => b.fields().id(),
            &License::Pair(ref p) => p.id()
        }
    }
    pub fn new(parsed: LicenseBlob) -> Result<License>{
        if parsed.fields().is_remote_placeholder(){
            Ok(License::Pair(LicensePair::new(parsed)?))
        }else{
            Ok(License::Single(parsed))
        }
    }
    pub fn is_pending(&self) -> bool{
        if let &License::Pair(ref p) = self{
            p.is_pending()
        }else {
            false
        }
    }

    pub fn first(&self) -> &LicenseBlob{
        match self {
            &License::Single(ref b) => b,
            &License::Pair(ref p) => &p.placeholder()
        }
    }

    pub fn fresh_remote(&self) -> Option<::parking_lot::RwLockReadGuard<Option<LicenseBlob>>>{
        match self {
            &License::Single(..) => None,
            &License::Pair(ref p) => Some(p.fresh_remote())
        }
    }

    pub fn dates(&self, manager: &LicenseManagerSingleton) -> SmallVec<[DateTime<FixedOffset>;4]>{
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



pub struct LicenseComputation<'mgr> {
    sink: IssueSink,
    mgr: &'mgr LicenseManagerSingleton,
    expires: Option<DateTime<FixedOffset>>,
    enforced: bool,
    licensed: bool,
}


pub enum LicenseScope<'a>{
    All,
    AllShared,
    List(::smallvec::SmallVec<[&'a str; 1]>)
}

impl<'a> LicenseScope<'a>{
    fn collect_from<'mgr>(&self, mgr: &'mgr LicenseManagerSingleton) -> SmallVec<[&'mgr License;1]>{
        match self{
            &LicenseScope::All => SmallVec::from_iter(mgr.iter_all()),
            &LicenseScope::AllShared => SmallVec::from_iter(mgr.iter_shared()),
            &LicenseScope::List(ref vec) =>
                SmallVec::from_iter(vec.iter().map(|id| mgr.get_by_id(id)).filter(|v|v.is_some()).map(|v| v.unwrap()))

        }
    }
}

impl<'mgr> LicenseComputation<'mgr>{

    pub fn licensed(&self) -> bool{
        self.licensed
    }

    fn build_date(&self) -> DateTime<FixedOffset>{
        self.mgr.clock().get_build_date()
    }

    fn is_build_date_ok(&self, license: &LicenseBlob) -> bool{
        if let Some(no_builds_after) = license.fields().subscription_expiration_date(){
            no_builds_after > self.build_date()
        }else{
            true
        }
    }
    fn is_license_expired(&self, license: &LicenseBlob) -> bool{
        if let Some(when) = license.fields().expires(){
            when < self.mgr.clock().get_utc_now().with_timezone(&when.timezone())
        }else {
            false
        }
    }
    fn has_license_begin(&self, license: &LicenseBlob) -> bool{
        if let Some(when) = license.fields().issued(){
            when < self.mgr.clock().get_utc_now().with_timezone(&when.timezone())
        }else {
            false
        }
    }

    fn validate_license(&mut self, license: &LicenseBlob) -> bool {
        if self.is_license_expired(license){
            self.sink.error(format!("License {} has expired.", license.id()), license.fields().to_redacted_str());
            return false;
        }
        if !self.has_license_begin(license){
            self.sink.error(format!("License {} was issued in the future; check system clock.", license.id()), license.fields().to_redacted_str());
            return false;
        }

        if !self.is_build_date_ok(license){
            let build_date = self.build_date();
            self.sink.error(format!("License {} covers Imageflow versions prior to {}, but you are using a build dated {:?}.", license.id(),
                            license.fields().subscription_expiration_date().unwrap().format("%F"),
                            build_date),
                            license.fields().to_redacted_str());
            return false;
        }
        if license.fields().is_revoked(){
            let message = license.fields().message().unwrap_or("license is no longer valid");
            self.sink.error(format!("License {}: {}.", license.id(), message),
                            license.fields().to_redacted_str());
            return false;
        }
        true
    }


    fn grace_period_for(&mut self, license: &License) -> Option<DateTime<Utc>>{
        if !self.validate_license(license.first()){
            return None;
        }

        let grace_minutes = license.first().fields().network_grace_minutes().unwrap_or(6);
        let expires = self.mgr.created() + ::time::Duration::minutes(grace_minutes as i64);

        if expires < self.mgr.clock().get_utc_now(){
            self.sink.error(format!("Grace period of {}m expired for license {}.", grace_minutes, license.id()),
            format!("License {} was not found in the disk cache and could not be retrieved from the remote server within {} minutes.", license.id(), grace_minutes));
            return None;
        }

        let thirty_seconds = self.mgr.created() + ::time::Duration::seconds(30);
        if thirty_seconds > self.mgr.clock().get_utc_now() {
            self.sink.warn(format!("Fetching license {} (not found in disk cache).", license.id()),
                           format!("Network grace period expires in {} minutes.", grace_minutes));
            return Some(thirty_seconds);
        }
        self.sink.error(format!("Grace period of {}m will expire for license {} at UTC {} on {}", grace_minutes, license.id(), expires.format("%H%M"), expires.format("%F")),
                        format!("License {} was not found in the disk cache and could not be retrieved from the remote server.", license.id()));
        Some(expires)

    }


    fn get_messages<'lic>(&self, license: &'lic LicenseBlob) -> SmallVec<[&'lic str;3]> {
        let array = [
            license.fields().message(),
            if self.is_license_expired(license) { license.fields().expiry_message() } else { None },
            license.fields().restrictions()
        ];

        SmallVec::from_iter(array
            .iter()
            .filter(|opt| opt.is_some() && !opt.unwrap().is_ascii_whitespace())
            .map(|opt| opt.unwrap()))
    }

    pub fn get_diagnostics(&self) -> String{
        let header = if self.enforced {
            "Licensing Enforced"
        } else {
            "Licensing not enforced"
        };

        let mut page = String::with_capacity(1024);
        page = page + header;
        // WIP
        page
    }

    fn validate_usage(&mut self, license: &LicenseBlob, required_features: &::smallvec::SmallVec<[&str;1]>) -> bool{
        if self.validate_license(license){
            let features = license.fields().features();
            let mut not_covered: SmallVec<[&str;1]> = SmallVec::new();
            for required in required_features{
                let search = UniCase::new(required);
                if features.iter().find(|f| f == &&search).is_none(){
                    not_covered.push(required);
                }
            }
            if not_covered.len() > 0{
                self.sink.error(format!("License {} needs to be upgraded; it does not cover in-use features {:?}", license.id(), not_covered), license.fields().to_redacted_str());
                false
            }else{
                true
            }
        }else{
            false
        }

    }

    pub fn new(mgr: &'mgr LicenseManagerSingleton,
               enforced: bool,
               scope: LicenseScope, required_features: &::smallvec::SmallVec<[&str;1]>) -> Self{

        let licenses = scope.collect_from(mgr);



        let mut c = LicenseComputation{
            mgr,
            enforced,
            expires: None,
            sink: IssueSink::new("LicenseComputation"),
            licensed: false,
        };

        let grace_periods: SmallVec<[DateTime<Utc>;1]> = SmallVec::from_iter(
            licenses
                .iter()
                .filter(|license| license.is_pending())
                .map(|license| c.grace_period_for(license))
                .filter(|period| period.is_some()).map(|period| period.unwrap())
        );

        c.expires = licenses
                .iter().flat_map(|license| license.dates(mgr))
                .chain(grace_periods.iter().map(|r| r.with_timezone(&FixedOffset::east(0))))
                .min();

        c.licensed = grace_periods.len() > 0;

        if !c.licensed{
            for license in licenses.iter()
                    .filter(|license| !license.is_pending()){

                if let Some(read) = license.fresh_remote(){
                    if let Some(ref remote) = *read{
                        if c.validate_usage(&remote, &required_features){
                            c.licensed = true;
                            break;
                        }
                    }
                }
                if let Some(remote) = c.mgr.cached_remote(license.id()){
                        if c.validate_usage(&remote, &required_features){
                            c.licensed = true;
                            break;
                        }
                }
                if c.validate_usage(license.first(), &required_features ){
                    c.licensed = true;
                    break;
                }
            }
        }

        c
    }
}
