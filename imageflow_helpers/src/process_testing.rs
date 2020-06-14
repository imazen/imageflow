use std;
use crate::preludes::from_std::*;
use std::process::{Command, Output, Stdio};
use super::timeywimey::Utc;


// create dir
// export sample json files
// export sample images
// test a couple remote URLs that we trust to work for years
// use a few embedded ones
// run imageflow in a few different ways


// #[derive(Clone, Debug, PartialEq)]
// enum ToolProduct{
//    CrashedUTF8{stderr: String, stdout: String},
//    NoErrorUTF8{stdout: String},
//    UserErrorUTF8{stdout: String, stderr: String, exit_code: i32},
//    GracefulBugUTF8{stdout: String, stderr: String, exit_code: i32},
//    Exited(Output),
//    Other(Output)
// }
//
//
// fn into_product(r: Output) -> ToolProduct{
//    let stdout_str = str::from_utf8(&r.stdout).map(|s| s.to_owned());
//    let stderr_str = str::from_utf8(&r.stderr).map(|s| s.to_owned());
//    if stderr_str.is_err() || stdout_str.is_err(){
//        if r.status.code() == None {
//            return ToolProduct::Other(r);
//        }else{
//            return ToolProduct::Exited(r);
//        }
//    }
//
//    let stderr_present = r.stderr.len() > 0;
//
//    match r.status.code(){
//        Some(65) => ToolProduct::UserErrorUTF8{ stdout: stdout_str.unwrap(), stderr: stderr_str.unwrap(),exit_code: r.status.code().unwrap()},
//        Some(70) => ToolProduct::GracefulBugUTF8{ stdout: stdout_str.unwrap(), stderr: stderr_str.unwrap(),exit_code: r.status.code().unwrap()},
//        None => ToolProduct::CrashedUTF8{ stdout: stdout_str.unwrap(), stderr: stderr_str.unwrap()},
//        Some(0) if !stderr_present => ToolProduct::NoErrorUTF8{ stdout: stdout_str.unwrap()},
//        Some(_) => ToolProduct::Exited(r)
//    }
// }
#[derive(Clone, Debug, PartialEq)]
pub struct ProcOutput {
    exit_code: Option<i32>,
    r: Option<Output>,
    empty: Vec<u8>
}

impl ProcOutput {
    pub fn from(r: Output) -> ProcOutput {
        ProcOutput { exit_code: r.status.code(),r: Some(r), empty: Vec::new() }
    }
    pub fn from_code(code: Option<i32>) -> ProcOutput {
        ProcOutput { r: None, exit_code: code, empty: Vec::new() }
    }
    //    fn status_code(&self) -> Option<i32> {
    //        match *self {
    //            ToolProduct::NoErrorUTF8 { .. } => Some(0),
    //            ToolProduct::UserErrorUTF8 { exit_code, .. } =>
    //                Some(exit_code),
    //            ToolProduct::GracefulBugUTF8 { exit_code, .. } =>
    //                Some(exit_code),
    //            ToolProduct::Exited(ref out) =>
    //                {out.status.code()}
    //            ToolProduct::Other(ref out) => {out.status.code()}
    //            ToolProduct::CrashedUTF8{ .. } => None
    //        }
    //    }

    fn stdout(&self) -> &Vec<u8>{
        self.r.as_ref().map(|r| &r.stdout).unwrap_or(&self.empty)
    }
    fn stderr(&self) -> &Vec<u8>{
        self.r.as_ref().map(|r| &r.stderr).unwrap_or(&self.empty)
    }

    pub fn status_code(&self) -> Option<i32> {
        self.exit_code
    }
    pub fn stdout_byte_count(&self) -> usize {
        self.stdout().len()
    }
    pub fn stdout_bytes(&self) -> &[u8] {
        self.stdout()
    }
    pub fn stderr_bytes(&self) -> &[u8] {
        self.stderr()
    }
    pub fn stderr_byte_count(&self) -> usize {
        self.stderr().len()
    }

    pub fn stderr_str(&self) -> &str {
        std::str::from_utf8(self.stderr())
            .expect("Implement lossy UTF-8 decoding for test results")
    }
    pub fn stdout_str(&self) -> &str {
        std::str::from_utf8(self.stdout())
            .expect("Implement lossy UTF-8 decoding for test results")
    }
    pub fn expect_exit_0_no_output(&self, m: &str) -> &ProcOutput {
        if self.stderr_byte_count() > 0 || self.stdout_byte_count() > 0 || self.status_code() != Some(0) {
            panic!("{}\nExpected exit code 0 and no output to stderr or stdout. Received\n {:?}\n{}",
                   m,
                   &self.r, std::str::from_utf8(self.stderr()).unwrap());
        }
        self
    }
    pub fn expect_status_code(&self, code: Option<i32>) -> &ProcOutput {
        if code != self.status_code(){
            self.dump();
            assert_eq!(code, self.status_code());
        }
        self
    }
    pub fn expect_stderr_contains(&self, substring: &str) -> &ProcOutput {
        if !self.stderr_str().contains(substring) {
            panic!("Failed to locate substring {:?} within stderr output {}",
                   substring,
                   self.stderr_str());
        }
        self
    }


    pub fn dump(&self) -> &ProcOutput {
        let _ = writeln!(&mut std::io::stderr(),
                         "Process output:\n{:?}\n{}\n{}\n",
                         self.r,
                            self.stdout_str(),
                         self.stderr_str());
        self
    }
}


pub struct ProcTestContext {
    exe: PathBuf,
    working_dir: PathBuf,
}

impl ProcTestContext {
    pub fn create_timestamp_subdir_within<P:AsRef<Path>>(parent_folder: P, exe_path: Option<PathBuf>) -> ProcTestContext {
        let working_dir = parent_folder.as_ref().join(format!("{:032}", Utc::now().timestamp()));
        Self::create(working_dir, exe_path)
    }
    pub fn bin_location(&self) -> &Path{
        self.exe.as_ref()
    }
    pub fn working_dir(&self) -> &Path{
        self.working_dir.as_ref()
    }
    pub fn create<P:AsRef<Path>>(working_dir: P, exe_path: Option<PathBuf>) -> ProcTestContext {
        let self_path = match exe_path {
            None => std::env::current_exe().expect("In order to test the executable, we need to know the binary's location. env::current_exe failed"),
            Some(p) => p,
        };
        if let Err(e) = create_dir_all(working_dir.as_ref()) {
            panic!("Failed to create directory {:?} due to {:?}", working_dir.as_ref(), e);
        }
        ProcTestContext {
            exe: self_path,
            working_dir: working_dir.as_ref().to_owned(),
        }
    }

    pub fn copy_ancestral_file_here<P:AsRef<Path>>(&self, filename: P) -> std::result::Result<(), String>{
        let mut dir = self.working_dir.canonicalize().map_err(|e| format!("{:?}", e))?;
        let to_path = self.working_dir.join(filename.as_ref());
        let mut last_err = None;

        if to_path.exists(){
            Ok(())
        }else {
            loop {
                let potential = dir.join(filename.as_ref());
                if potential.exists() {
                    match std::fs::copy(potential.as_path(), to_path.as_path()) {
                        Ok(_) => {
                            return Ok(());
                        },
                        Err(e) => {
                            //Try another ancestor if the copy failed.
                            last_err = Some(e);
                        }
                    }
                }

                dir = match dir.parent() {
                    Some(v) => v.to_owned(),
                    None => { break; }
                }
            }
            Err(format!("Failed to locate {:?} in ancestors of {:?}. err({:?})", filename.as_ref(), &dir, last_err))
        }
    }

    pub fn create_valgrind_suppressions(&self) -> std::result::Result<(), String>{
        self.copy_ancestral_file_here("valgrind_suppressions.txt")?;
        self.copy_ancestral_file_here(".valgrindrc")
    }
    pub fn subfolder_context<P:AsRef<Path>>(&self, subfolder: P) -> ProcTestContext {
        let new_dir = self.working_dir.join(subfolder.as_ref());
        if let Err(e) = create_dir_all(&new_dir) {
            panic!("Failed to create directory {:?} due to {:?}", &new_dir, e);

        }
        ProcTestContext {
            working_dir: new_dir,
            exe: self.exe.clone()
        }
    }


    pub fn exec(&self, args: &str) -> ProcOutput{
        let args = args.split_whitespace().collect::<Vec<&str>>();
        self.execute(args, true, |_| {})
    }
    // Parsing may not be unix compliant;
    pub fn exec_full(&self, full_invocation: &str) -> ProcOutput {
        let mut args = full_invocation.split_whitespace().collect::<Vec<&str>>();
        let _ = args.remove(0);
        self.execute(args, true, |_| {})
    }

    ///
    /// Pass false for valgrind_on_signal_death if your callback might kill the child
    pub fn execute<F>(&self, args_vec: Vec<&str>, valgrind_on_signal_death: bool, callback: F) -> ProcOutput
                where F: Fn(&mut std::process::Child) -> () {

        //TODO: serialize in a safer way - this isn't correct
        let full_invocation = format!("{} {}", &self.exe.to_str().unwrap(), args_vec.join(" "));

        let dir = self.working_dir.as_path();
        let exe = self.exe.as_path();

        let valgrind_copy_result = self.create_valgrind_suppressions();
        let _ = writeln!(&mut std::io::stderr(),
                         "Executing from folder {} with valgrind_suppressions {:?}\n{}",
                         dir.to_str().unwrap(),
                         valgrind_copy_result,
                         full_invocation);
        // change working dir to dir
        let mut cmd = Command::new(exe);
        cmd.args(args_vec.as_slice()).current_dir(dir).env("RUST_BACKTRACE", "1");


        cmd.stderr(Stdio::piped()).stdout(Stdio::piped());

        let mut child_process = match cmd.spawn(){
            Ok(v) => v,
            Err(e) => {
                panic!("Failed to start {:?} {:?} error: {:?}", &exe, &cmd, e)
            }
        };


        callback(&mut child_process);

        let output: Output = child_process.wait_with_output().unwrap();
        let _ = writeln!(&mut std::io::stderr(),
                         "exit code {:?}", output.status.code());

        // Try to debug segfaults
        if output.status.code() == None && valgrind_on_signal_death{

            std::io::stderr().write_all(&output.stderr).unwrap();
            std::io::stdout().write_all(&output.stdout).unwrap();
            let _ = writeln!(&mut std::io::stderr(),
                             "exit code {:?}", output.status.code());
            // Killed by signal.
            // 11 Segmentation fault
            // 4 illegal instruction 6 abort 8 floating point error

            if std::env::var("VALGRIND_RUNNING").is_ok() {
                let _ = writeln!(&mut std::io::stderr(),
                                 "VALGRIND_RUNNING defined; skipping valgrind pass");
            }else{
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

        ProcOutput::from(output)
    }




    pub fn write_file<P:AsRef<Path>>(&self, filename: P, bytes: &[u8]){
        let path = self.working_dir.join(filename);
        let mut file = BufWriter::new(File::create(&path).unwrap());
        file.write_all(bytes).unwrap();
    }

    pub fn read_file_str<P:AsRef<Path>>(&self, filename: P) -> String{
        let path = self.working_dir.join(filename);
        let mut file = File::open(&path).unwrap();
        let mut contents = String::new();
        file.read_to_string( &mut contents).unwrap();
        contents
    }
}

