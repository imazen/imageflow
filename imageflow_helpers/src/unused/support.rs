use chrono::prelude::*;
use std::borrow::Borrow;
use std::hash::{Hash, Hasher};
use super::*;
use super::super::util::*;


pub struct IssueSink{
    source: &'static str,
    dict: HashMap<u64, Issue>
}
impl IssueSink {
    pub fn new(source: &'static str) -> Self {
        IssueSink {
            source,
            dict: HashMap::new()
        }
    }
    pub fn error(&mut self, msg: String, detail: String) {
        let issue = Issue::new(IssueKind::Error, msg, detail, self.source);

        eprintln!("{}\n", &issue);
        self.dict.insert(issue.hash(), issue);
    }

    pub fn warn(&mut self, msg: String, detail: String) {
        let issue = Issue::new(IssueKind::Warning, msg, detail, self.source);
        self.dict.insert(issue.hash(), issue);
    }
    pub fn message(&mut self, msg: String, detail: String) {
        let issue = Issue::new(IssueKind::Message, msg, detail, self.source);
        self.dict.insert(issue.hash(), issue);
    }
}



impl fmt::Display for IssueSink {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for issue in self.dict.values(){
            write!(f,"{}\n", issue)?;
        }
        Ok(())
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
