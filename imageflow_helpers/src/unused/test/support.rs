use chrono::prelude::*;

use super::super::super::util::*;
use super::super::*;

fn parse_date_as_utc(s: &str) -> DateTime<Utc>{
    DateTime::from_utc(NaiveDateTime::new(
        NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("date must be valid"),
        NaiveTime::from_hms(0,0,0)),
                       Utc)
}
#[derive(Debug,Clone,Copy)]
pub struct OffsetClock{
    pub build_date: DateTime<Utc>,
    pub ticks_offset_ns: u64,
    pub offset: ::chrono::Duration,
}
impl OffsetClock{
    pub fn new(date: &str, build_date: &str) -> OffsetClock{
        OffsetClock{
            offset: Utc::now().signed_duration_since(parse_date_as_utc(date)),
            ticks_offset_ns: crate::timeywimey::precise_time_ns() -1,
            build_date: parse_date_as_utc(build_date).with_timezone(&Utc)
        }
    }
}
impl AppClock for OffsetClock{
    fn get_timestamp_ticks(&self) -> u64 {

        crate::timeywimey::precise_time_ns() - self.ticks_offset_ns
    }

    fn ticks_per_second(&self) -> u64 {
        100000000
    }

    fn get_build_date(&self) -> DateTime<Utc> {
        self.build_date
    }

    fn get_utc_now(&self) -> DateTime<Utc> {
        Utc::now() - self.offset
    }
}

pub struct StringMemCache{
    cache: ::chashmap::CHashMap<String, String>,
}
impl StringMemCache{
    pub fn new() -> Self{
        StringMemCache{
            cache: ::chashmap::CHashMap::new()
        }
    }
    pub fn into_cache(self) -> Box<dyn PersistentStringCache + Send + Sync>{
        Box::new(self)
    }
}
impl PersistentStringCache for StringMemCache{
    fn try_put(&self, key: &String, value: &str) -> StringCachePutResult {
        if let Some(old) = self.cache.insert(key.to_owned(), value.to_owned()){
            if old == value{
                return StringCachePutResult::Duplicate;
            }
        }
        return StringCachePutResult::WriteComplete;
    }

    fn get(&self, key: &String) -> Option<String> {
        self.cache.get(key).map(|v| v.to_owned())
    }
}
