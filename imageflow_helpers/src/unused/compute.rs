#![allow(bad_style)]
#![allow(unused, deprecated)]
use super::*;
use std::ascii::AsciiExt;
use smallvec::SmallVec;
use std::iter::FromIterator;
use super::super::util::*;


pub enum EnforcementMethod{ None, Watermark, Error}


pub struct LicenseReportInfo<'a>{
    enforcement: EnforcementMethod,
    relevant_domains: Vec<&'a str>,
    public_report: bool,
}

impl<'a>  LicenseReportInfo<'a>{
    fn create_header<'mgr>(&self, c: &mut LicenseComputation<'mgr>) -> Option<String>{
        Some(String::new())
    }

    fn create_body<'mgr>(&self, c: &mut LicenseComputation<'mgr>) -> Option<String>{
        Some(String::new())
    }
}

//
//pub fn license_status_summary(&self){
//    if self.licensed_all() {
//        "License valid for all domains"
//    } else {
//
//    }
//}
//pub fn get_header(&self, include_sales: bool, include_scope: bool) -> String{
//    let header = if self.enforced {
//        "------------------------- Licensing ON -------------------------\r\n"
//    } else {
//        "------------------------- Licensing OFF -------------------------\r\n"
//    };
//
//    let mut page = String::with_capacity(1024);
//    page = page + header + "\n\n\n";
//
//
//    page = page + &format!("{}", self.sink);
//    // WIP
//    page
//}

pub struct LicenseComputation<'mgr> {
    sink: Option<IssueSink>,
    mgr: &'mgr LicenseManagerSingleton,
    expires: Option<DateTime<Utc>>,
    licensed_some: bool,
    licensed_all: bool,
    licensed_domains: SmallVec<[UniCase<String>;2]>,
    report_header: Option<String>,
    report_body: Option<String>
}


pub enum LicenseScope<'a>{
    All,
    AllShared,
    IdList(::smallvec::SmallVec<[&'a str; 1]>)
}

impl<'a> LicenseScope<'a>{
    fn collect_from<'mgr>(&self, mgr: &'mgr LicenseManagerSingleton) -> SmallVec<[&'mgr License;1]>{
        match self{
            &LicenseScope::All => SmallVec::from_iter(mgr.iter_all()),
            &LicenseScope::AllShared => SmallVec::from_iter(mgr.iter_shared()),
            &LicenseScope::IdList(ref vec) =>
                SmallVec::from_iter(vec.iter().map(|id| mgr.get_by_id(id)).filter(|v|v.is_some()).map(|v| v.unwrap()))

        }
    }
}

impl<'mgr> LicenseComputation<'mgr>{

    pub fn licensed(&self) -> bool{
        self.licensed_some
    }
    pub fn licensed_all(&self) -> bool{
        self.licensed_all
    }
    pub fn licensed_domains(&self) -> &SmallVec<[UniCase<String>;2]>{
        &self.licensed_domains
    }


    fn build_date(&self) -> DateTime<Utc>{
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
            when < self.mgr.clock().get_utc_now()
        }else {
            false
        }
    }
    fn has_license_begin(&self, license: &LicenseBlob) -> bool{
        if let Some(when) = license.fields().issued(){
            when < self.mgr.clock().get_utc_now()
        }else {
            false
        }
    }

    fn get_messages<'lic>(&self, license: &'lic LicenseBlob) -> SmallVec<[&'lic str;3]> {
        let array = [
            license.fields().message(),
            if self.is_license_expired(license) { license.fields().expiry_message() } else { None },
            license.fields().restrictions()
        ];

        SmallVec::from_iter(array
            .iter()
            .filter_map(|opt| opt.and_then(|v| if v.chars().all(|c|c.is_ascii_whitespace()) { None } else { Some(v) }))
        )
    }

    fn validate_license(&mut self, license: &LicenseBlob) -> bool{
        let messages = self.get_messages(license);
        for m in messages {
            self.sink.do_some(|s| s.message(format!("License {}: {}", license.id(), m), String::new()));
        }

        if self.is_license_expired(license){
            self.sink.do_some(|s| s.error(format!("License {} has expired.", license.id()), license.fields().to_redacted_str()));
            return false;
        }
        if !self.has_license_begin(license){
            self.sink.do_some(|s| s.error(format!("License {} was issued in the future; check system clock.", license.id()), license.fields().to_redacted_str()));
            return false;
        }

        if !self.is_build_date_ok(license){
            let build_date = self.build_date();
            self.sink.do_some(|s| s.error(format!("License {} covers Imageflow versions prior to {}, but you are using a build dated {:?}.", license.id(),
                            license.fields().subscription_expiration_date().unwrap().format("%F"),
                            build_date),
                            license.fields().to_redacted_str()));
            return false;
        }
        if license.fields().is_revoked(){
            let message = license.fields().message().unwrap_or("license is no longer valid");
            self.sink.do_some(|s| s.error(format!("License {}: {}.", license.id(), message),
                            license.fields().to_redacted_str()));
            return false;
        }
        true
    }

    fn validate_grace_period(&mut self, license: &License) -> Option<DateTime<Utc>>{
        if !self.validate_license(license.first()){
            return None;
        }

        let grace_minutes = license.first().fields().network_grace_minutes().unwrap_or(6);
        let expires = self.mgr.created() + ::chrono::Duration::minutes(grace_minutes as i64);

        if expires < self.mgr.clock().get_utc_now(){
            self.sink.do_some(|s| s.error(format!("Grace period of {}m expired for license {}.", grace_minutes, license.id()),
            format!("License {} was not found in the disk cache and could not be retrieved from the remote server within {} minutes.", license.id(), grace_minutes)));
            return None;
        }

        let thirty_seconds = self.mgr.created() + ::chrono::Duration::seconds(30);
        if thirty_seconds > self.mgr.clock().get_utc_now() {
            self.sink.do_some(|s| s.warn(format!("Fetching license {} (not found in disk cache).", license.id()),
                           format!("Network grace period expires in {} minutes.", grace_minutes)));
            return Some(thirty_seconds);
        }
        self.sink.do_some(|s| s.error(format!("Grace period of {}m will expire for license {} at UTC {} on {}", grace_minutes, license.id(), expires.format("%H%M"), expires.format("%F")),
                        format!("License {} was not found in the disk cache and could not be retrieved from the remote server.", license.id())));
        Some(expires)

    }

    fn validate_blob_usage(&mut self, license: &LicenseBlob, required_features: &::smallvec::SmallVec<[&str;1]>) -> bool{
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
                self.sink.do_some(|s| s.error(format!("License {} needs to be upgraded; it does not cover in-use features {:?}", license.id(), not_covered), license.fields().to_redacted_str()));
                false
            }else{
                true
            }
        }else{
            false
        }

    }

    /// Attempt to validate remote, then cached, then placeholder. Stop validating on success, and return domain names (if any)
    /// Allocates to own domain names
    fn validate_license_usage(&mut self, license: &License, required_features: &::smallvec::SmallVec<[&str;1]>) -> Option<SmallVec<[UniCase<String>;2]>>{
        if license.is_pair() {
            if let Some(read) = license.fresh_remote() {
                if let Some(ref remote) = *read {
                    if self.validate_blob_usage(&remote, &required_features) {
                        return Some(remote.fields().domains_owned());
                    }
                }
            }
            if let Some(remote) = self.mgr.cached_remote(license.id()) {
                if self.validate_blob_usage(&remote, &required_features) {
                    return Some(remote.fields().domains_owned());
                }
            }
        } else if self.validate_blob_usage(license.first(), &required_features) {
            return Some(license.first().fields().domains_owned());
        }
        None
    }

    pub fn new<'a>(mgr: &'mgr LicenseManagerSingleton,
               info: Option<&'a LicenseReportInfo>,
               scope: LicenseScope, required_features: &::smallvec::SmallVec<[&str;1]>) -> Self{

        let compute_started = mgr.clock().get_utc_now();

        let licenses = scope.collect_from(mgr);

        let mut c = LicenseComputation{
            mgr,
            expires: None,
            sink: if info.is_some() { Some( IssueSink::new("License Computation") ) } else { None },
            licensed_some: false,
            licensed_all: false,
            licensed_domains: SmallVec::new(),
            report_header: None,
            report_body: None,
        };

        // .validate_grace_period() validates all pending licenses
        let grace_periods: SmallVec<[DateTime<Utc>;1]> = SmallVec::from_iter(
            licenses
                .iter()
                .filter(|license| license.is_pair() && !license.remote_fetched())
                .filter_map(|license| c.validate_grace_period(license))
        );

        let grace_period_active = !grace_periods.is_empty();

        // Take the nearest future issued, expired, or grace period expiration dates
        c.expires = licenses
                .iter().flat_map(|license| license.dates(mgr))
                .chain(grace_periods.into_iter())
                .filter(|date| date > &compute_started)
                .min();

        // Validate all non-pending licenses even if we already have authorization (so that all the appropriate warnings are logged to the sink)
        for license in licenses {
            if let Some(domains) = c.validate_license_usage(license, &required_features){
                if domains.is_empty() {
                    c.licensed_all = true;
                }else {
                    c.licensed_domains.extend(domains);
                }
            }
        }

        c.licensed_all = c.licensed_all || grace_period_active;
        c.licensed_some = !c.licensed_domains.is_empty() || c.licensed_all;

        if let Some(info) = info{
            c.report_header = info.create_header(&mut c);
            c.report_body = info.create_body(&mut c);
        }
        c
    }
}
