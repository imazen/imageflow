use ::chrono::{DateTime, Utc};
use ::std;
use ::std::any::Any;
use ::std::fmt;

/// Debounce filter with interval adjustment and prediction (not Sync/Send)
pub struct Debounce {
    interval: Ticks,
    next: Ticks,
    ticks_per_second: Ticks,
}
impl Debounce {
    pub fn new(interval: ::chrono::Duration, clock: &dyn AppClock) -> Debounce {
        let mut d = Debounce { interval: 0, next: 0, ticks_per_second: clock.ticks_per_second() };
        d.set_interval(interval);
        d
    }
    pub fn interval(&self) -> ::chrono::Duration {
        ::chrono::Duration::nanoseconds(
            (self.interval as u128 * self.ticks_per_second as u128 / 1000000000) as i64,
        )
    }
    pub fn set_interval(&mut self, interval: ::chrono::Duration) {
        self.interval = (interval.num_nanoseconds().unwrap_or(0) as u128
            * self.ticks_per_second as u128
            / 1000000000) as u64;
    }

    pub fn set_interval_if_stopped(&mut self, interval: ::chrono::Duration) {
        if self.interval == 0 {
            self.set_interval(interval);
        }
    }

    pub fn stop(&mut self) {
        self.interval = 0;
    }
    pub fn next(&self) -> Ticks {
        self.next
    }
    pub fn allow(&mut self, clock: &dyn AppClock) -> bool {
        if self.interval == 0 {
            false
        } else {
            let now = clock.get_timestamp_ticks();
            if now > self.next {
                self.next = now + self.interval;
                true
            } else {
                false
            }
        }
    }
}

/// Like .for_each, but for Option instead of Iterator
pub trait DoSome {
    type Item;
    fn do_some<F>(&mut self, f: F) -> ()
    where
        F: FnMut(&mut Self::Item);
}
impl<T> DoSome for Option<T> {
    type Item = T;
    fn do_some<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Self::Item),
    {
        if let Some(ref mut v) = *self {
            f(v)
        }
    }
}

/// A time, ticks, and build date source for
pub trait AppClock: Sync + Send {
    fn get_timestamp_ticks(&self) -> u64;
    fn ticks_per_second(&self) -> u64;
    fn get_build_date(&self) -> DateTime<Utc>;
    fn get_utc_now(&self) -> DateTime<Utc>;
}

#[derive(Debug, Clone, Copy)]
pub struct DefaultClock {
    pub build_date: DateTime<Utc>,
}
impl AppClock for DefaultClock {
    fn get_timestamp_ticks(&self) -> u64 {
        crate::timeywimey::precise_time_ns()
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

/// Allow a &str or String panic value to be printed
pub struct PanicFormatter<'a>(pub &'a dyn Any);
impl<'a> std::fmt::Display for PanicFormatter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(str) = self.0.downcast_ref::<String>() {
            writeln!(f, "panicked: {}", str)?;
        } else if let Some(str) = self.0.downcast_ref::<&str>() {
            writeln!(f, "panicked: {}", str)?;
        }
        Ok(())
    }
}

pub type Ticks = u64;

#[derive(Debug, PartialEq)]
pub enum IssueKind {
    Error,
    Warning,
    Message,
}

/// Represents a problem that should be relayed in diagnostic reports and possibly printed to stderr
/// Deduplicated by hash of the message (detail is excluded)
#[derive(Debug, PartialEq)]
pub struct Issue {
    hash: u64,
    source: &'static str,
    message: String,
    detail: String,
    kind: IssueKind,
}

impl Issue {
    pub fn new(kind: IssueKind, message: String, detail: String, source: &'static str) -> Self {
        let hash = crate::hashing::hash_64(message.as_bytes());
        Issue { hash, message, detail, kind, source }
    }
    pub fn hash(&self) -> u64 {
        self.hash
    }
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {:?}: {}\n{}\n", self.source, self.kind, self.message, self.detail)
    }
}
