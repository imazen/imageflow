#![feature(slice_concat_ext)]
#![feature(integer_atomics)]

extern crate imageflow_helpers;
extern crate imageflow_core as fc;
extern crate imageflow_types as s;
use ::imageflow_helpers::preludes::from_std::*;
extern crate hyper;

use std::slice::SliceConcatExt;

extern crate wait_timeout;
use wait_timeout::ChildExt;
use std::time::Duration;
use std::process::{Command, Stdio, Child,Output};

use ::imageflow_helpers::process_testing::*;
use fc::test_helpers::*;
use fc::test_helpers::process_testing::ProcTestContextExtras;
use fc::test_helpers::process_testing::ProcOutputExtras;
use ::imageflow_helpers::fetching::FetchError;
use ::imageflow_helpers::fetching::{fetch_bytes,get_status_code_for};
fn server_path() -> PathBuf{
    let self_path = std::env::current_exe().expect("For --self-test to work, we need to know the binary's location. env::current_exe failed");
    self_path.parent().unwrap().join("imageflow_server")
}

fn get_next_port() -> u16{
    use std::sync::atomic::{AtomicU16, Ordering, ATOMIC_U16_INIT};
    static NEXT_PORT: AtomicU16 = ATOMIC_U16_INIT;

    NEXT_PORT.compare_and_swap(0, 36703, Ordering::SeqCst);
    NEXT_PORT.fetch_add(1, Ordering::SeqCst)
}

struct ServerInstance{
    port: u16,
    protocol: Proto
}

type CallbackResult = std::result::Result<(), ::imageflow_helpers::fetching::FetchError>;

#[derive(Debug,PartialEq,Eq,Copy,Clone)]
enum Proto{
    Http,
    Https
}
impl ServerInstance{

    fn speaks_http(&self) -> bool{
        self.hello().is_ok()
    }

    fn hello(&self) -> std::result::Result<hyper::status::StatusCode, FetchError> {
        get_status_code_for(&self.url_for("/hello/are/you/running?"))
    }

    fn url_for(&self, rel_path: &str) -> String{
        if self.protocol == Proto::Https{
            format!("https://localhost:{}{}", self.port,rel_path)
        }else{
            format!("http://localhost:{}{}", self.port,rel_path)
        }

    }

    fn get_status(&self, rel_path: &str) -> std::result::Result<hyper::status::StatusCode, FetchError> {
        get_status_code_for(&self.url_for(rel_path))
    }


    fn request_stop(&self) -> std::result::Result<hyper::status::StatusCode, FetchError> {
        get_status_code_for(&self.url_for("/test/shutdown"))
    }



    fn run<F>(c: &ProcTestContext, protocol: Proto, args: Vec<&str>, callback: F) -> (ProcOutput, CallbackResult)
    where F: Fn(&ServerInstance) -> CallbackResult {
        let instance = ServerInstance {
            port: get_next_port(),
            protocol: protocol
        };
        // NOTE --bind=localhost::{} (two colons) causes a generic "error:",exit code 1, and no other output. This is bad UX.
        let test_arg = "--integration-test";
        let port_arg = format!("--port={}", instance.port);
        let cert_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(Path::new("src")).join(Path::new("assets")).join(Path::new("identity.p12"));

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
                           |child: &mut std::process::Child| -> std::result::Result<(), ::imageflow_helpers::fetching::FetchError> {

                               ::std::thread::sleep_ms(500);
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

    let context = ProcTestContext::create_timestamp_subdir_within("server_tests", Some(server_path()));

    {
        let c = context.subfolder_context("basics");
        c.exec("diagnose --show-compilation-info").expect_status_code(Some(0));
        c.exec("--version").expect_status_code(Some(0));
        c.exec("-V").expect_status_code(Some(0));

        //TODO: test diagnose --call-panic (xplat hard)

        //Test incorrect args
        c.execute(vec!["demo"], false, |child: &mut std::process::Child| {
        }).expect_status_code(Some(1));

    }

    {
        let c = context.subfolder_context("demo"); //stuck on port 39876
        c.subfolder_context("demo");
        let (po, callback_result) = ServerInstance::run(&c, Proto::Http, vec!["--demo", "--data-dir=."], | server | {
            fetch_bytes(&server.url_for("/ir4/proxy_unsplash/photo-1422493757035-1e5e03968f95?width=100"))?;
            assert_eq!(server.get_status("/ir4/proxy_unsplash/notthere.jpg")?, hyper::status::StatusCode::NotFound);
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

        let (po, callback_result) = ServerInstance::run(&c, Proto::Http, params , | server | {
            let bytes = fetch_bytes(&server.url_for("/local/eh.png?width=100")).unwrap();

            let info = fc::clients::stateless::LibClient {}.get_image_info(&bytes).expect("Image response should be valid");


            for ix in 1..last_mount + 1{

                let url = format!("/local_{ix}/eh2.png?w=1", ix=ix);
                println!("Testing {}", &url);
                let bytes = fetch_bytes(&server.url_for(&url)).unwrap();

                let info = fc::clients::stateless::LibClient {}.get_image_info(&bytes).expect("Image response should be valid");

                assert_eq!(server.get_status(&url)?, hyper::status::StatusCode::Ok);
            }


            assert_eq!(server.get_status("/local/notthere.jpg")?, hyper::status::StatusCode::NotFound);
            assert_eq!(server.get_status("/notrouted")?, hyper::status::StatusCode::NotFound);
            Ok(())
        });
        //po.expect_status_code(Some(0));

        callback_result.unwrap();
    }
    {
        let c = context.subfolder_context("https_demo"); //stuck on port 39876
        c.subfolder_context("demo");
        let (po, callback_result) = ServerInstance::run(&c, Proto::Https, vec!["--demo", "--data-dir=."], | server | {
            fetch_bytes(&server.url_for("/ir4/proxy_unsplash/photo-1422493757035-1e5e03968f95?width=100"))?;
            assert_eq!(server.get_status("/ir4/proxy_unsplash/notthere.jpg")?, hyper::status::StatusCode::NotFound);
            Ok(())
        });

        //po.expect_status_code(Some(0));

        callback_result.unwrap();
    }
}
#[test]
fn run_server_test_ir4_heavy(){
    let context = ProcTestContext::create_timestamp_subdir_within("server_tests_heavy", Some(server_path()));
    {
        let c = context.subfolder_context("mount_local_test"); //stuck on port 39876
        c.exec("diagnose --show-compilation-info").expect_status_code(Some(0));
        c.create_blank_image_here("eh", 100,100, s::EncoderPreset::libpng32());

        let mut params = vec!["--data-dir=.", "--mount=/local/:ir4_local:./"];
        let (po, callback_result) = ServerInstance::run(&c, Proto::Http, params , | server | {
            for ix in 1..20{
                let bytes = fetch_bytes(&server.url_for("/local/eh.png?width=100")).unwrap();
                let info = fc::clients::stateless::LibClient {}.get_image_info(&bytes).expect("Image response should be valid");
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


        ///child.kill().unwrap();
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
                std::io::stderr().write(&out.stderr).unwrap();
                std::io::stdout().write(&out.stdout).unwrap();
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