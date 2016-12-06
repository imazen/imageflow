extern crate std;
use fc::for_other_imageflow_crates::preludes::default::*;
extern crate imageflow_core as fc;
extern crate chrono;
extern crate curl;
extern crate os_type;
extern crate imageflow_types;
extern crate zip;
//

use std::process::{Command, Output};


//
//
//fn exec_full(&self, full_invocation: &str) -> ToolProduct {
//let mut parts_vec = full_invocation.split_whitespace().collect::<Vec<&str>>();
//let _ = parts_vec.remove(0);
//
//let args_vec = parts_vec;
//let dir = self.test_dir.as_path();
//let exe = self.imageflow_tool.as_path();
//
//let valgrind_copy_result = self.create_valgrind_suppressions();
//let _ = writeln!(&mut std::io::stderr(),
//"Executing from folder {} with valgrind_suppressions {:?}\n{}",
//dir.to_str().unwrap(),
//valgrind_copy_result,
//full_invocation);
//// change working dir to dir
//let mut cmd = Command::new(exe);
//cmd.args(args_vec.as_slice()).current_dir(dir).env("RUST_BACKTRACE", "1");
//let output: Output = cmd.output().expect("Failed to start?");
//let _ = writeln!(&mut std::io::stderr(),
//"exit code {:?}", output.status.code());
//
//// Try to debug segfaults
//if output.status.code() == None {
//
//
//std::io::stderr().write(&output.stderr).unwrap();
//std::io::stdout().write(&output.stdout).unwrap();
//println!("exit code {:?}", output.status.code());
//
//// Killed by signal.
//// 11 Segmentation fault
//// 4 illegal instruction 6 abort 8 floating point error
//let _ = writeln!(&mut std::io::stderr(),
//"Starting valgrind from within self-test:");
//let mut cmd = Command::new("valgrind");
//cmd.arg("-q").arg("--error-exitcode=9").arg(exe);
//cmd.args(args_vec.as_slice()).current_dir(dir).env("RUST_BACKTRACE", "1");
//
//println!("{:?}", cmd);
//
//let _ = cmd.status(); //.expect("Failed to start valgrind?");
//}
//
//into_product(output)
//}


pub struct CaptureTo{
    args: Vec<String>,
    executable: PathBuf,
    basepath: String,
}

// stay minimal
// --capture-to basename
// Runs itself, setting RUST_BACKTRACE=1, capturing stdout/stderr to basename_stdout/err.txt
// Writes basename_run.bat/.sh (invocation) and basename_info.txt (version and build info). writes basename_info.json for automated tooling
// Copies target executable to basename_imageflow_tool
// Captures current operating system info

impl CaptureTo{
    pub fn create_default(capture_to: &str,  args: Vec<String>) -> CaptureTo{
        CaptureTo::create(capture_to, None, args)
    }

    pub fn create(capture_to: &str, bin_location: Option<PathBuf>, args: Vec<String>) -> CaptureTo{
        let executable= bin_location.unwrap_or_else(|| std::env::current_exe().expect("For --capture-to to work, we need to know the binary's location. env::current_exe failed"));

        CaptureTo{
            args: args,
            executable: executable,
            basepath: capture_to.to_owned()
        }

    }
    fn write_bytes(&self, suffix: &str, bytes: &[u8]) -> std::result::Result<(),std::io::Error>{
        let filename = format!("{}_{}", self.basepath, suffix);
        let mut file = BufWriter::new(File::create(&filename)?);
        file.write(bytes).and_then(|_| Ok(()))
    }

    fn run_and_save_output_to(&self, suffix: &str, args: &[&str]) -> std::result::Result<(),std::io::Error>{
        let mut cmd = Command::new(&self.executable);
        cmd.args(args).env("RUST_BACKTRACE","1");
        let output = cmd.output()?;

        let filename = format!("{}_{}", self.basepath, suffix);
        let mut file = BufWriter::new(File::create(&filename)?);

        let header = format!("{:?} exited with status {:?}\nSTDERR:\n", cmd, output.status);
        file.write(&header.into_bytes())?;
        file.write(&output.stderr)?;

        let header = format!("\n\n\nSTDOUT:\n");
        file.write(&header.into_bytes())?;
        file.write(&output.stdout)?;
        Ok(())
    }
    pub fn run(&self) -> (){

        let mut cmd = Command::new(&self.executable);
        cmd.args(&self.args).env("RUST_BACKTRACE", "1");

        let invocation = format!("{:?}",cmd).into_bytes();
        self.write_bytes("run.txt", &invocation).unwrap();

        let output: Output = cmd.output().unwrap(); //Better, log the ioError

        //Shouldn't we verify it's not a command-line syntax error?
//        match output.status.code(){
//            Some(0) => {
//                //Was this an incorrect result?
//            }
//            Some(128)
//
//        }

        let status_file = format!("exitcode_{:?}.txt", &output.status.code());
        self.write_bytes(&status_file, &[]).unwrap();

        self.write_bytes("stdout.txt", &output.stdout).unwrap();
        self.write_bytes("stderr.txt", &output.stderr).unwrap();
        self.run_and_save_output_to("version.txt",&["--version"]).unwrap();
        self.run_and_save_output_to("compilation_info.txt",&["diagnose", "--show-compilation-info"]).unwrap();
        //To many bytes. Maybe just the summary, not the folder?
        //self.run_and_save_output_to("self-test.txt",&["diagnose", "--self-test"]).unwrap();

        //If it is expected to be stored, we just save the URL
        if let  &Some(url) = imageflow_types::version::get_build_env_value("ESTIMATED_ARTIFACT_URL"){
            self.write_bytes("artifact_url.txt", url.as_bytes()).unwrap();
        }else{
            //Otherwise copy the binary
            let target_path = format!("{}_{}", self.basepath, self.executable.as_path().file_name().unwrap().to_str().unwrap());
            std::fs::copy(&self.executable, &target_path).unwrap();
        }



        //TODO: get local operating system information
        ()
    }
    pub fn exit_code(&self) -> i32 {
        0
    }
}

pub fn zip_directory_nonrecursive(dir: &Path, archive_name: &Path) -> zip::result::ZipResult<()> {
    let mut zip = zip::ZipWriter::new(File::create(archive_name).unwrap());

    let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.add_directory(archive_name.file_stem().unwrap().to_str().unwrap().to_owned(), options)?;
    let entries = std::fs::read_dir(dir).unwrap();

    for entry_maybe in entries {
        if let Ok(entry) = entry_maybe {
            let file_name = entry.file_name().into_string().unwrap();
            if file_name.starts_with(".") {
                //skipping
            } else {
                if entry.path().is_file() {
                    let mut file = File::open(entry.path()).unwrap();
                    let mut contents = Vec::new();
                    file.read_to_end(&mut contents).unwrap();

                    let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

                    zip.start_file(file_name, options)?;
                    zip.write_all(&contents)?;
                }
            }
        }
        //println!("Name: {}", path.unwrap().path().display())
    }

    zip.finish()?;

    Ok(())
}