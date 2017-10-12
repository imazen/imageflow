use chrono::prelude::*;
use std::borrow::Borrow;
use std::hash::{Hash, Hasher};
use super::*;


#[derive(Debug,PartialEq)]
pub enum IssueKind{ Error, Warning}

#[derive(Debug, PartialEq)]
pub struct Issue{
    hash: u64,
    source: &'static str,
    message: String,
    detail: String,
    kind: IssueKind,
}

impl Issue{
    pub fn new(kind: IssueKind, message: String, detail:String, source: &'static str) -> Self{
        let hash = ::hashing::hash_64(message.as_bytes());
        Issue{
            hash,
            message,
            detail,
            kind,
            source
        }
    }
}
pub struct IssueSink{
    source: &'static str,
    dict: HashMap<u64, Issue>
}
impl IssueSink{
    pub fn new(source: &'static str) -> Self{
        IssueSink{
            source,
            dict: HashMap::new()
        }
    }
    pub fn error(&mut self, msg: String, detail: String){
        let issue = Issue::new(IssueKind::Error,msg, detail, self.source);
        self.dict.insert(issue.hash, issue);
    }

    pub fn warn(&mut self, msg: String, detail: String){
        let issue = Issue::new(IssueKind::Warning,msg, detail, self.source);
        self.dict.insert(issue.hash, issue);
    }

}

//#[allow(dead_code)]
//pub struct IssueSync{
//    source: &'static str,
//    dict: ::chashmap::CHashMap<u64, Issue>
//}
//impl IssueSync {
//    pub fn new(source: &'static str) -> Self {
//        IssueSync {
//            source,
//            dict: ::chashmap::CHashMap::new()
//        }
//    }
//    pub fn error(&self, msg: String, detail: String) {
//        let issue = Issue::new(IssueKind::Error,msg, detail);
//        self.dict.insert(issue.hash, issue);
//    }
//    pub fn warn(&self, msg: String, detail: String) {
//        let issue = Issue::new(IssueKind::Warning,msg, detail);
//        self.dict.insert(issue.hash, issue);
//    }
//}

#[derive(Debug,Clone,Copy)]
struct DefaultClock{
    pub build_date: DateTime<Utc>
}
impl LicenseClock for DefaultClock{
    fn get_timestamp_ticks(&self) -> u64 {
        ::time::precise_time_ns()
    }

    fn ticks_per_second(&self) -> u64 {
        100000000
    }

    fn get_build_date(&self) -> DateTime<Utc> {
        self.build_date
    }

    fn get_utc_now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

// TODO: uncomment this when support lands in rustc for improved safety
//#[repr(transparent)]
#[derive(Copy,Clone,Debug)]
pub struct AsciiFolding<S: ?Sized>(S);

impl<S> AsciiFolding<S> {
    pub fn new(s: S) -> Self {
       AsciiFolding(s)
    }
}

impl<S: AsRef<str>> AsRef<str> for AsciiFolding<S> {
    #[inline]
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl<S: fmt::Display> fmt::Display for AsciiFolding<S> {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, fmt)
    }
}

impl<S: ?Sized> AsciiFolding<S> {
    pub fn borrowed(s: &S) -> &Self {
        unsafe { &*(s as *const S as *const AsciiFolding<S>) }
    }
}

impl Borrow<AsciiFolding<str>> for AsciiFolding<String> {
    #[inline]
    fn borrow(&self) -> &AsciiFolding<str> {
        AsciiFolding::borrowed(self.0.borrow())
    }
}

impl<S: Borrow<str> + ?Sized> PartialEq for AsciiFolding<S> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.borrow().eq_ignore_ascii_case(other.0.borrow())
    }
}

impl<S: Borrow<str> + ?Sized> Eq for AsciiFolding<S> {}

impl<S: Borrow<str> + ?Sized> Hash for AsciiFolding<S> {
    #[inline]
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        for byte in self.0.borrow().bytes().map(|b| b.to_ascii_lowercase()) {
            hasher.write_u8(byte);
        }
    }
}

#[test]
fn test_hashmap_key() {
    let mut m = std::collections::HashMap::new();
    m.insert(AsciiFolding("v".to_owned()), "value");

    m.get(AsciiFolding::borrowed("v")).unwrap();
}
