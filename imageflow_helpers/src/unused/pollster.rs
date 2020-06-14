// Single thread
// shared data structure for adding endpoints
// Endpoints callback
use crate::preludes::from_std::*;
use ::std;
use std::thread;
use std::thread::JoinHandle;
use ::parking_lot::Mutex;
use std::panic::AssertUnwindSafe;
use super::unused::support::IssueSink;
use smallvec::SmallVec;
// Get build date
// Get ticks
// Get utcnow
use std::sync::Arc;
use std::sync::atomic::{Ordering, AtomicBool};
use super::util::*;

pub trait Endpoint{
    fn redact(&self, str: &mut str);
    fn get_fetch_interval(&self) -> ::chrono::Duration;
    fn get_query(&self) -> ::std::result::Result<&str, String>;
    fn get_path(&self) -> ::std::result::Result<&str, String>;
    fn get_base_urls(&self) -> SmallVec<[Cow<'static, str>;6]>;
    fn process_response(&self, content_type: Option<&::reqwest::header::HeaderValue>, bytes: Vec<u8>) -> ::std::result::Result<(), String>;
}


pub struct EndpointEntry{
    endpoint: Box<dyn Endpoint>,
    sleep_until: Ticks,
    interval: Debounce,
    error_interval: Debounce,
}
impl EndpointEntry{
    pub fn new(e: Box<dyn Endpoint>, clock: &dyn AppClock) -> EndpointEntry{
        let interval = Debounce::new(e.get_fetch_interval(), clock);
        EndpointEntry{
            endpoint: e,
            sleep_until: 0,
            interval,
            error_interval: Debounce::new(::chrono::Duration::nanoseconds(0), clock)
        }
    }
}

pub struct FetcherConfig<'clock>{
    sink: IssueSink,
    client: ::reqwest::Client,
    clock: &'clock dyn AppClock,
    initial_error_interval: ::chrono::Duration,
}

#[cfg(not(test))]
fn mock_swap_base_urls(v: SmallVec<[Cow<'static, str>;6]>) -> SmallVec<[Cow<'static, str>;6]> {
    v
}
#[cfg(test)]
fn mock_swap_base_urls(_v: SmallVec<[Cow<'static, str>;6]>) -> SmallVec<[Cow<'static, str>;6]> {
    let mut nv = SmallVec::new();
    nv.push(Cow::from(::mockito::server_url()));
    nv
}

impl<'clock> FetcherConfig<'clock>{
    pub fn new(clock: &'clock dyn AppClock) -> Self{
        FetcherConfig{
            client: ::reqwest::Client::new(),
            initial_error_interval: ::chrono::Duration::seconds(3),
            clock,
            sink: IssueSink::new("Fetcher")
        }
    }

    fn fetch(&mut self, e: &mut EndpointEntry, token: Arc<SharedToken>, is_error_retry: bool) {
        if let Err(mut err) = self.fetch_inner(e, token,is_error_retry){
            e.endpoint.redact(&mut err);
            self.sink.error(err, String::new());
            e.error_interval.set_interval_if_stopped(self.initial_error_interval);
        }
    }

    fn fetch_inner(&mut self, e: &mut EndpointEntry,token: Arc<SharedToken>, _is_error_retry: bool) -> ::std::result::Result<(),String>{

        let path = e.endpoint.get_path().map_err(|mut s| { s.insert_str(0,".get_path() failed:"); s })?;
        let query = e.endpoint.get_query().map_err(|mut s| { s.insert_str(0,".get_query() failed:"); s })?;

        let mut responses: SmallVec<[(i32,String);6]> = SmallVec::new();

        let base_urls = e.endpoint.get_base_urls();
        if base_urls.is_empty(){
            responses.push((-3, "No base URLs provided".to_owned()));
        }

        let base_urls = mock_swap_base_urls(base_urls);
        for base in base_urls{
            if token.canceled(){
                return Ok(())
            }
            let url_str = format!("{}{}{}", base, path, query);
            match ::reqwest::Url::from_str(&url_str) {
                Ok(url) => {
                    match self.client.request(::reqwest::Method::GET, url).send() {
                        Ok(mut response) => {
                            if response.status().is_success() {
                                let mut bytes = Vec::new();
                                response.read_to_end(&mut bytes).unwrap();

                                let content_type = response.headers().get(::reqwest::header::CONTENT_TYPE).clone();

                                println!("Fetched {}", &url_str);
                                match e.endpoint.process_response(content_type, bytes) {
                                    Ok(()) => {
                                        e.error_interval.stop();
                                        token.increment_fetches();
                                        return Ok(());
                                    },
                                    Err(err) => {
                                        responses.push((-3, format!(".process_response() failed for {}\n{}", &url_str, err)));
                                    }
                                }
                            }else{
                                responses.push((i32::from(response.status().as_u16()), format!("Server returned {} for {}", response.status(), &url_str)));
                            }
                        },
                        Err(err) => {
                            responses.push((-2, format!("Failed to reach server; ensure firewall permits GET {}\n{}", &url_str, err)));
                        }
                    }
                },
                Err(err) => {
                    responses.push((-1, format!("Failed to parse url {}\n{}", &url_str, err)));
                }
            }
        }

        let mut message = format!("Failed to fetch {}", path);
        let mut detail = String::with_capacity(responses.len() * 255);
        for r in responses.iter(){
            detail.push_str(&r.1);
            detail.push('\n');
        }

        e.endpoint.redact(&mut message);
        e.endpoint.redact(&mut detail);
        self.sink.error(message, detail);

        // let mut e_interval = e.error_interval.interval();
        e.error_interval.set_interval_if_stopped(self.initial_error_interval);

        /*
        if is_error_retry && !e_interval.is_zero(){
            e_interval = ::chrono::Duration::milliseconds((e_interval.num_milliseconds() as f32 * self.error_interval_multiplier) as i64);
            use rand::Rng;
            e_interval = e_interval + ::chrono::Duration::milliseconds(::rand::thread_rng().gen_range(0, 2000) );

            // Error interval maxes out at 233% of regular check interval
            if e_interval > e.interval.interval(){
                e_interval = e.interval.interval() * 7 / 3;
            }
        }*/

        // Increment the fetch count if we got any HTTP response whatsoever
        if responses.iter().any(|p| p.0 > 0){
            token.increment_fetches();
        }

        Ok(())
    }
    fn work_endpoint(&mut self, e: &mut EndpointEntry, token: Arc<SharedToken>) -> Ticks{
        if e.sleep_until > self.clock.get_timestamp_ticks(){
            return e.sleep_until;
        }
        if e.interval.allow(self.clock) {
            self.fetch(e, token,false)
        } else if e.error_interval.allow(self.clock){
            self.fetch(e, token,true)
        }

        e.sleep_until = cmp::min(e.error_interval.next(), e.interval.next());
        return e.sleep_until;
    }
}

pub struct SharedToken{
    licenses_fetched: ::parking_lot::Mutex<usize>,
    licenses_fetched_change: ::parking_lot::Condvar,
    cancel: ::parking_lot::Mutex<bool>,
    cancel_change: ::parking_lot::Condvar,
    cancel_light: AtomicBool,
    shutdown_requested: AtomicBool,
    thread_handle: Mutex<Option<JoinHandle<()>>>,
    shutdown_finished: ::parking_lot::Mutex<bool>,
    shutdown_finished_change: ::parking_lot::Condvar,

}
impl SharedToken{
    pub fn new() -> Self{
        SharedToken{
            licenses_fetched: ::parking_lot::Mutex::new(0),
            licenses_fetched_change: ::parking_lot::Condvar::new(),
            cancel: ::parking_lot::Mutex::new(false),
            cancel_change: ::parking_lot::Condvar::new(),
            cancel_light: AtomicBool::new(false),
            shutdown_requested: AtomicBool::new(false),
            shutdown_finished: ::parking_lot::Mutex::new(false),
            shutdown_finished_change: ::parking_lot::Condvar::new(),
            thread_handle: Mutex::new(None),
        }
    }
    /// Wait until the given number of successful fetches have been recorded
    pub fn wait_for_fetches(&self, fetch_count: usize){
        let mut fetched = self.licenses_fetched.lock();
        while *fetched < fetch_count{
            self.licenses_fetched_change.wait(&mut fetched);
        }
    }
    pub fn wait_for_fetches_ms(&self, fetch_count: usize, timeout_milliseconds: u64) -> bool{
        let started_at = crate::timeywimey::precise_time_ns();
        let mut fetched = self.licenses_fetched.lock();
        while *fetched < fetch_count{
            let now = crate::timeywimey::precise_time_ns();
            let wait_until = started_at + timeout_milliseconds * 1000000;
            if wait_until < now {
                return *fetched >= fetch_count;
            }
            if self.licenses_fetched_change.wait_for(&mut fetched, ::std::time::Duration::from_millis((wait_until - now) / 1000000)).timed_out(){
                return *fetched >= fetch_count;
            }
        }
        return true;
    }

    /// Increase the number of fetches
    pub fn increment_fetches(&self){
        eprint!("fetches++");
        let mut fetched = self.licenses_fetched.lock();
        *fetched = *fetched + 1;
        self.licenses_fetched_change.notify_one();
    }
    /// Request the fetcher reboot
    pub fn request_cancel(&self){
        self.cancel_light.store(true, Ordering::SeqCst);
        let mut c = self.cancel.lock();
        *c = true;
        self.cancel_change.notify_one();
    }
    /// Request the fetcher begin shutting down
    pub fn request_shutdown(&self){
        self.shutdown_requested.store(true, Ordering::SeqCst);
        self.request_cancel();
    }

    /// Tell everyone that the shutdown is complete; the thread can be joined.
    pub fn shutdown_complete(&self){
        let mut c = self.shutdown_finished.lock();
        *c = true;
        self.shutdown_finished_change.notify_one();
    }

    /// True if a shutdown has been requested
    pub fn shutdown_requested(&self) -> bool{
        self.shutdown_requested.load(Ordering::SeqCst)
    }


    pub(crate) fn join_thread(self) -> std::thread::Result<()>{
        self.request_shutdown();
        if let Some(h) = self.thread_handle.into_inner(){
            h.join()
        }else{
            Ok(())
        }

    }

    /// Reset the cancellation
    pub fn reset_cancel(&self){
        let mut c = self.cancel.lock();
        *c = false;
        self.cancel_light.store(false, Ordering::SeqCst);
        self.cancel_change.notify_one();
    }

    /// Waits until cancellation is requested
    pub fn wait_for_cancel(&self, timeout: ::std::time::Duration) -> bool{
        let mut canceled = self.cancel.lock();
        *canceled || (!self.cancel_change.wait_for(&mut canceled, timeout).timed_out() && *canceled)
    }


    /// Returns true if shutdown succeeds, false if timeout occurs.
    pub fn wait_for_shutdown(&self, timeout: ::std::time::Duration) -> bool{
        self.request_shutdown();
        let mut c = self.shutdown_finished.lock();
        *c ||  (!self.shutdown_finished_change.wait_for(&mut c, timeout).timed_out() && *c)
    }

    /// True if cancellation has been requested
    pub fn canceled(&self) -> bool{
        self.cancel_light.load(Ordering::Relaxed)
    }

    // Does not restart after shutdown
    pub fn ensure_spawned<F>(&self, f: F)
        where
            F: FnOnce() -> (),
            F: Send + 'static {

        let mut handle = self.thread_handle.lock();
        if (*handle).is_none(){
            *handle = Some(thread::spawn(f));
        }
    }
}

pub struct Fetcher<'clock>{
    config: FetcherConfig<'clock>,
    endpoints: Vec<EndpointEntry>,
}

impl<'clock> Fetcher<'clock>{

    pub fn new(endpoints: Vec<Box<dyn Endpoint>>, clock: &'clock dyn AppClock) -> Self{
        Fetcher{
            config: FetcherConfig::new(clock),
            endpoints: endpoints.into_iter().map(|e| EndpointEntry::new(e, clock)).collect()
        }
    }


    fn main(&mut self, token: Arc<SharedToken>) {
        while !token.canceled() {
            let mut wake_at = u64::max_value();
            for endpoint in self.endpoints.iter_mut() {
                wake_at = cmp::min(self.config.work_endpoint(endpoint, token.clone()), wake_at);
                if token.canceled(){
                    return;
                }
            }
            let ticks_now = self.config.clock.get_timestamp_ticks();
            if wake_at > ticks_now {
                let sleep_seconds = cmp::max(1, (wake_at - ticks_now) / self.config.clock.ticks_per_second());
                eprintln!("Sleeping for {}s", sleep_seconds);
                if token.wait_for_cancel(std::time::Duration::from_secs(sleep_seconds)) {
                    return;
                }
            }
        }
    }


    pub fn ensure_spawned<F>(token: Arc<SharedToken>, clock: Arc<dyn AppClock>, endpoint_producer: F)
        where F: Fn() -> Vec<Box<dyn Endpoint>>, F: Send + 'static {

        token.clone().ensure_spawned(move || {
            eprintln!("starting thread");
            while !token.shutdown_requested(){
                token.reset_cancel();
                let token_clone = token.clone();
                let endpoints = endpoint_producer();
                let clock_clone = clock.clone();
                let result = ::std::panic::catch_unwind( AssertUnwindSafe(move || {
                    let mut fetcher = Fetcher::new(endpoints, clock_clone.as_ref());
                    fetcher.main(token_clone);
                }));
                if let Err(e) = result{
                    eprintln!("{}", PanicFormatter(&e));
                }
            }
            eprintln!("finishing thread");
            // Mark the shutdown as complete
            token.shutdown_complete();
        });
    }
}
