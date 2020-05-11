
extern crate imageflow_helpers;
extern crate imageflow_core as fc;
extern crate imageflow_types as s;
use imageflow_helpers::preludes::from_std::*;
extern crate hyper;


extern crate wait_timeout;
use wait_timeout::ChildExt;
use std::time::Duration;
use std::process::{Command, Stdio, Output};
use std::net::{TcpListener};

#[macro_use]
extern crate lazy_static;

use std::sync::Mutex;

use imageflow_helpers::process_testing::*;
use crate::fc::test_helpers::process_testing::ProcTestContextExtras;
use ::imageflow_http_helpers::{fetch, fetch_bytes,get_status_code_for, FetchError, FetchConfig};

use std::collections::vec_deque::VecDeque;
use reqwest::StatusCode;

lazy_static! {
    static ref RECENT_PORTS: Mutex<VecDeque<u16>> = Mutex::new(VecDeque::new());
}

fn assert_valid_image(url: &str) {
    match fetch(url, Some(FetchConfig{ custom_ca_trust_file: None, read_error_body: Some(true)})){
        Ok(v) => {
            fc::clients::stateless::LibClient {}.get_image_info(&v.bytes).expect("Image response should be valid");
        },
        Err(e) => { panic!("{:?} for {}", &e, &url); }
    }
}
//fn write_env_vars(path: &Path){
//    let mut f = File::create(&path).unwrap();
//    for (k,v) in std::env::vars(){
//        write!(f, "{}={}\n", k, v).unwrap();
//    }
//}

fn build_dirs() -> Vec<PathBuf>{
    let target_triple = crate::s::version::get_build_env_value("TARGET").expect("TARGET triple required");
    let profile = crate::s::version::get_build_env_value("PROFILE").expect("PROFILE (debug/release) required");


    let target_dir = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("target");

    let a = target_dir.join(target_triple).join(profile);
    let b = target_dir.join(profile);
    vec![a,b]
}
#[cfg(windows)]
fn binary_ext() -> &'static str{
    "exe"
}
#[cfg(not(windows))]
fn binary_ext() -> &'static str{
    ""
}

fn locate_binary(name: &str) -> Option<PathBuf> {
    for dir in build_dirs() {
        let file_path = dir.join(name).with_extension(binary_ext());

        if file_path.exists() {
            return Some(dir.join(name))
        }
    }
    None
}


fn server_path() -> PathBuf {
   match locate_binary("imageflow_server"){
       Some(v) => v,
       None => {
           panic!("Failed to locate imageflow_server binary in {:?}", build_dirs());
       }
   }
}

fn enqueue_unique_port(q: &mut VecDeque<u16>, count: usize) -> u16{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    if !q.contains(&port) {
        q.push_back(port);
        if count <= 1{
             port
        }else {
            enqueue_unique_port(q, count - 1)
        }
    } else {
        enqueue_unique_port(q, count)
    }
}

fn fetch_next_port(q: &mut VecDeque<u16>) -> u16 {
    if q.len() < 1 {
        let _ = enqueue_unique_port(q, 25); // pre-check ports 25 at a time.
    }
    q.pop_front().unwrap()
}

fn get_next_port() -> u16 {
    let mut q = RECENT_PORTS.lock().unwrap();
    fetch_next_port(&mut q)
}


struct ServerInstance{
    port: u16,
    protocol: Proto,
    #[allow(dead_code)]
    trust_ca_file: Option<PathBuf>,
    #[allow(dead_code)]
    cert: Option<PathBuf>

}

type CallbackResult = std::result::Result<(), ::imageflow_http_helpers::FetchError>;

#[derive(Debug,PartialEq,Eq,Copy,Clone)]
enum Proto{
    Http,
    Https
}
impl ServerInstance{

    fn hello(&self) -> Result<StatusCode, FetchError> {
        get_status_code_for(&self.url_for("/hello/are/you/running?"))
    }

    fn url_for(&self, rel_path: &str) -> String{
        if self.protocol == Proto::Https{
            format!("https://localhost:{}{}", self.port,rel_path)
        }else{
            format!("http://localhost:{}{}", self.port,rel_path)
        }

    }

    fn get_status(&self, rel_path: &str) -> Result<StatusCode, FetchError> {
        get_status_code_for(&self.url_for(rel_path))
    }

    fn request_stop(&self) -> Result<StatusCode, FetchError> {
        get_status_code_for(&self.url_for("/test/shutdown"))
    }

    fn run<F>(c: &ProcTestContext, protocol: Proto, args: Vec<&str>, callback: F) -> (ProcOutput, CallbackResult)
    where F: Fn(&ServerInstance) -> CallbackResult {
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join(Path::new("src")).join(Path::new("assets"));
        let cert_path = assets.join(Path::new("identity.p12"));
        let ca_path = assets.join(Path::new("root-ca.pem"));


        let instance = ServerInstance {
            port: get_next_port(),
            protocol,
            trust_ca_file: Some(ca_path),
            cert: Some(cert_path.clone())
        };
        // NOTE --bind=localhost::{} (two colons) causes a generic "error:",exit code 1, and no other output. This is bad UX.
        let test_arg = "--integration-test";
        let port_arg = format!("--port={}", instance.port);

        let mut all_args = args.clone();
        if protocol == Proto::Https{
            all_args.insert(0, "mypass");
            all_args.insert(0, "--certificate-password");
            all_args.insert(0, cert_path.to_str().unwrap());
            all_args.insert(0, "--certificate");
        }
        all_args.insert(0, test_arg);
        all_args.insert(0, &port_arg);
        all_args.insert(0, "start");

        c.execute_callback(all_args, false,
                           |_child: &mut std::process::Child| -> std::result::Result<(), ::imageflow_http_helpers::FetchError> {

                               ::std::thread::sleep(::std::time::Duration::from_millis(500));
                               // Server may not be running
                               instance.hello()?;

                               let r = callback(&instance);

                               let _ = instance.request_stop();
                               r
                           })
        //po.expect_status_code(Some(0));
    }
}


// ports 36,000 to 39,999 seem the safest.
#[test]
fn run_server_test_i4(){

    //write_env_vars(&Path::new("env.txt"));

    let context = ProcTestContext::create_timestamp_subdir_within(std::env::current_dir().unwrap().join("server_tests"), Some(server_path()));

    {
        let c = context.subfolder_context("basics");
        c.exec("diagnose --show-compilation-info").expect_status_code(Some(0));
        c.exec("--version").expect_status_code(Some(0));
        c.exec("-V").expect_status_code(Some(0));

        //TODO: test diagnose --call-panic (xplat hard)

        //Test incorrect args
        c.execute(vec!["demo"], false, |_child: &mut std::process::Child| {
        }).expect_status_code(Some(1));

    }

    {
        let c = context.subfolder_context("demo"); //stuck on port 39876
        c.subfolder_context("demo");
        let (_po, callback_result) = ServerInstance::run(&c, Proto::Http, vec!["--demo", "--data-dir=."], | server | {
            fetch_bytes(&server.url_for("/ir4/proxy_unsplash/photo-1422493757035-1e5e03968f95?width=100"))?;
            //TODO: Find a way to test upstream 404 and 403 errors
            // assert_eq!(server.get_status("/demo_images/notthere.jpg")?, http::StatusCode::NOT_FOUND);

            let url = server.url_for("/proxied_demo/index.html");
            match fetch(&url, Some(FetchConfig{ custom_ca_trust_file: None, read_error_body: Some(true)})){
                Ok(_) => {},
                Err(e) => { panic!("{:?} for {}", &e, &url); }
            }

            assert_valid_image(&server.url_for("/demo_images/example-028-whitespace.jpg?width=600&trim.threshold=80&trim.percentpadding=0.5"));


            Ok(())
        });

        //po.expect_status_code(Some(0));

        callback_result.unwrap();
    }
    {
        let c = context.subfolder_context("proxy");
        c.subfolder_context("proxy");
        let (_po, callback_result) = ServerInstance::run(&c, Proto::Http, vec!["--data-dir=.", "--mount","/extern/:ir4_http:http:://images.unsplash.com/"], | server | {
            fetch_bytes(&server.url_for("/extern/photo-1422493757035-1e5e03968f95?width=100"))?;
            Ok(())
        });

        //po.expect_status_code(Some(0));

        callback_result.unwrap();
    }
    {
        let c = context.subfolder_context("mount_local"); //stuck on port 39876
        c.create_blank_image_here("eh", 100,100, s::EncoderPreset::libpng32());
        let a = c.subfolder_context("a"); //stuck on port 39876
        a.create_blank_image_here("eh2", 100,100, s::EncoderPreset::libpng32());

        let mut params = vec!["--data-dir=.", "--mount=/local/:ir4_local:./",
                              "--mount=/local_1/:ir4_local:./a",
                              "--mount=/local_2/:ir4_local:./a/",
                              "--mount=/local_3/:ir4_local:a"];
        if std::path::MAIN_SEPARATOR == '\\'{
            params.push(r"--mount=/local_4/:ir4_local:.\a");
            params.push(r"--mount=/local_5/:ir4_local:.\a/");
            params.push(r"--mount=/local_6/:ir4_local:.\a\");
        }

        let last_mount = params.len() - 2;

        let (_, callback_result) = ServerInstance::run(&c, Proto::Http, params , | server | {
            assert_valid_image(&server.url_for("/local/eh.png?width=100"));

            for ix in 1..last_mount + 1{
                let url = format!("/local_{ix}/eh2.png?w=1", ix=ix);
                println!("Testing {}", &url);
                assert_valid_image(&server.url_for(&url));
            }


            assert_eq!(server.get_status("/local/notthere.jpg")?, StatusCode::NOT_FOUND);
            assert_eq!(server.get_status("/notrouted")?, StatusCode::NOT_FOUND);
            Ok(())
        });
        //po.expect_status_code(Some(0));

        callback_result.unwrap();
    }

    // we can't currently test https server support. We *should* be able to, on linux - but ... nope.
    //test_https(context);
}

#[allow(dead_code)]
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn test_https(context: &ProcTestContext){
    {
        let c = context.subfolder_context("https_demo"); //stuck on port 39876
        c.subfolder_context("demo");
        let (_, callback_result) = ServerInstance::run(&c, Proto::Https, vec!["--demo", "--data-dir=."], | server | {
            let url = server.url_for("/ir4/proxy_unsplash/photo-1422493757035-1e5e03968f95?width=100");
            let bytes = fetch(&url, Some(FetchConfig{custom_ca_trust_file: server.trust_ca_file.clone(), read_error_body: Some(true) })).expect(&url).bytes;
            let _ = fc::clients::stateless::LibClient {}.get_image_info(&bytes).expect("Image response should be valid");

            //assert_eq!(server.get_status("/ir4/proxy_unsplash/notthere.jpg")?, http::StatusCode::NOT_FOUND);
            Ok(())
        });

        //po.expect_status_code(Some(0));

        callback_result.unwrap();
    }
}

#[allow(dead_code)]
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn test_https(_context: ProcTestContext){}

#[test]
fn run_server_test_ir4_heavy(){
    let context = ProcTestContext::create_timestamp_subdir_within(std::env::current_dir().unwrap().join("server_tests_heavy"), Some(server_path()));
    {
        let c = context.subfolder_context("mount_local_test"); //stuck on port 39876
        c.exec("diagnose --show-compilation-info").expect_status_code(Some(0));
        c.create_blank_image_here("eh", 100,100, s::EncoderPreset::libpng32());

        let params = vec!["--data-dir=.", "--mount=/local/:ir4_local:./"];
        let (_, callback_result) = ServerInstance::run(&c, Proto::Http, params , | server | {
            for _ in 1..20{
                assert_valid_image(&server.url_for("/local/eh.png?width=100"));
            }
            Ok(())
        });
        callback_result.unwrap();
    }
}

trait ProcTestContextHttp{
 fn execute_callback<F,T>(&self, args_vec: Vec<&str>, valgrind_on_signal_death: bool, callback: F) -> (ProcOutput, T)
where F: Fn(&mut std::process::Child) -> T;

}
impl ProcTestContextHttp for ProcTestContext{

    ///
    /// Pass false for valgrind_on_signal_death if your callback might kill the child
    fn execute_callback<F,T>(&self, args_vec: Vec<&str>, valgrind_on_signal_death: bool, callback: F) -> (ProcOutput, T)
        where F: Fn(&mut std::process::Child) -> T {
        //TODO: serialize in a safer way - this isn't correct
        let full_invocation = format!("{} {}", &self.bin_location().to_str().unwrap(), args_vec.join(" "));

        let dir = self.working_dir();
        let exe = self.bin_location();

        let valgrind_copy_result = self.create_valgrind_suppressions();
        let _ = writeln!(&mut std::io::stderr(),
                         "Executing from folder {} with valgrind_suppressions {:?}\n{}",
                         dir.to_str().unwrap(),
                         valgrind_copy_result,
                         full_invocation);
        // change working dir to dir
        let mut cmd = Command::new(exe);
        cmd.args(args_vec.as_slice()).current_dir(dir).env("RUST_BACKTRACE", "1");


        //cmd.stderr(Stdio::piped()).stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit()).stdout(Stdio::inherit());


        let mut child = cmd.spawn().expect("Failed to start?");


        let result = callback(&mut child);


        //child.kill().unwrap();
        let timeout = Some(Duration::from_secs(1));

        let (status_code, output) = match timeout {
            Some(timeout) => {
                match child.wait_timeout(timeout).unwrap() {
                    Some(status) => (status.code(), None),
                    None => {
                        // child hasn't exited yet
                        child.kill().unwrap();
                        (child.wait().unwrap().code(), None)
                    }
                }
            }
            None => {
                let output: Output = child.wait_with_output().unwrap();
                (output.status.code(), Some(output))
            }
        };

        let _ = writeln!(&mut std::io::stderr(),
                         "exit code {:?}", status_code);

        // Double check we dumped output on segfault
        if status_code == None {
            if let Some(ref out) = output {
                std::io::stderr().write_all(&out.stderr).unwrap();
                std::io::stdout().write_all(&out.stdout).unwrap();
            }
            let _ = writeln!(&mut std::io::stderr(),
                             "exit code {:?}", status_code);
        }
        // Killed by signal.
        // 11 Segmentation fault
        // 4 illegal instruction 6 abort 8 floating point error
        if status_code == None && valgrind_on_signal_death {
            if std::env::var("VALGRIND_RUNNING").is_ok() {
                let _ = writeln!(&mut std::io::stderr(),
                                 "VALGRIND_RUNNING defined; skipping valgrind pass");
            } else {
                //ALLOW TO FAIL; valgrind may not be present
                let _ = writeln!(&mut std::io::stderr(),
                                 "Starting valgrind from within self-test:");
                let mut cmd = Command::new("valgrind");
                cmd.arg("-q").arg("--error-exitcode=9").arg(exe);
                cmd.args(args_vec.as_slice()).current_dir(dir).env("RUST_BACKTRACE", "1").env("VALGRIND_RUNNING", "1");

                let _ = writeln!(&mut std::io::stderr(),
                                 "{:?}", cmd);

                let _ = cmd.status(); //.expect("Failed to start valgrind?");
            }
        }

        match output {
            Some(out) => (ProcOutput::from(out), result),
            None => (ProcOutput::from_code(status_code), result)
        }
    }

}
