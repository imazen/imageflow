use chrono::prelude::*;
use time::Duration;
use super::*;


#[derive(Debug,PartialEq)]
pub enum IssueKind{ Error, Warning}

#[derive(Debug, PartialEq)]
pub struct Issue{
    hash: u64,
    message: String,
    detail: String,
    kind: IssueKind,
}

impl Issue{
    pub fn new(kind: IssueKind, message: String, detail:String) -> Self{
        let hash = ::hashing::hash_64(message.as_bytes());
        Issue{
            hash,
            message,
            detail,
            kind
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
        let issue = Issue::new(IssueKind::Error,msg, detail);
        self.dict.insert(issue.hash, issue);
    }

    pub fn warn(&mut self, msg: String, detail: String){
        let issue = Issue::new(IssueKind::Warning,msg, detail);
        self.dict.insert(issue.hash, issue);
    }
}


pub struct IssueSync{
    source: &'static str,
    dict: ::chashmap::CHashMap<u64, Issue>
}
impl IssueSync {
    pub fn new(source: &'static str) -> Self {
        IssueSync {
            source,
            dict: ::chashmap::CHashMap::new()
        }
    }
    pub fn error(&self, msg: String, detail: String) {
        let issue = Issue::new(IssueKind::Error,msg, detail);
        self.dict.insert(issue.hash, issue);
    }
    pub fn warn(&self, msg: String, detail: String) {
        let issue = Issue::new(IssueKind::Warning,msg, detail);
        self.dict.insert(issue.hash, issue);
    }
}

#[derive(Debug,Clone,Copy)]
struct DefaultClock{
    pub build_date: DateTime<FixedOffset>
}
impl LicenseClock for DefaultClock{
    fn get_timestamp_ticks(&self) -> u64 {
        ::time::precise_time_ns()
    }

    fn ticks_per_second(&self) -> u64 {
        100000000
    }

    fn get_build_date(&self) -> DateTime<FixedOffset> {
        self.build_date
    }

    fn get_utc_now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

