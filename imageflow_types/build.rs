#[macro_use]
extern crate quick_error;

use std::env;
use std::convert::AsRef;
use std::fs::{File, create_dir_all};
use std::io::{Write, Read, BufWriter};
use std::path::Path;
use std::process::Command;
use std::collections::HashMap;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Io(err: std::io::Error) {
            from()
        }
        MissingEnvVar {
        }
    }
}
//
//struct CompilationInfo{
//    env_vars: HashMap<&'static str, Option<String>>,
//    commands: HashMap<&'static str, Option<String>>,
//    conan_info: String,
//
//}
//
static ENV_VARS: [&'static str;17] = ["GIT_COMMIT", "GIT_COMMIT_SHORT","GIT_OPTIONAL_TAG",
    "GIT_DESCRIBE_ALWAYS", "GIT_DESCRIBE_ALWAYS_LONG", "GIT_DESCRIBE_AAL", "GIT_OPTIONAL_BRANCH",
    "ESTIMATED_ARTIFACT_URL","ESTIMATED_DOCS_URL","CI_SEQUENTIAL_BUILD_NUMBER","CI_BUILD_URL","CI_JOB_URL","CI_JOB_TITLE","CI_STRING",
    "CI_PULL_REQUEST_INFO", "CI_TAG", "CI_RELATED_BRANCH"
];

//fn add_command(&mut commands: HashMap<&'static str, Option<String>>, key: &'static str, command: &str){
//
//}
//fn populate_compilation_info() -> CompilationIfno{
//    let mut c = HashMap::new();
//    add_command(c, "GIT_COMMIT", "git rev-parse HEAD");
//    add_command(c, "GIT_COMMIT_SHORT", "git rev-parse --short HEAD");
//    add_command(c, "GIT_DESCRIBE_ALWAYS", "git describe --always --tags");
//    add_command(c, "GIT_DESCRIBE_ALWAYS_LONG", "git describe --always --tags --long");
//    add_command(c, "GIT_DESCRIBE_AAL", "git describe --always --all --long");
//    add_command(c, "GIT_OPTIONAL_TAG", "git describe --exact-match --tags");
//    add_command(c, "GIT_OPTIONAL_BRANCH", "git rev-parse --abbrev-ref HEAD");
//    add_command(c, "CONAN_VERSION", "conan -V");
//    add_command(c, "DEFAULT_GCC_VERSION", "gcc -v");
//    add_command(c, "DEFAULT_CLANG_VERSION", "clang --version");
//    add_command(c, "DEFAULT_RUSTC_VERSION", "rustc- -V");
//    add_command(c, "DEFAULT_CARGO_VERSION", "cargo -V");
//
//
//    let mut env = HashMap::new();
//    for name in ENV_VARS.iter(){
//        if let Some(value) = env::var(name).ok(){
//            env.insert(name, value);
//        }
//    }
//
//
//    //    - git rev-parse --short HEAD | set /P GIT_COMMIT_SHORT
//    //        - git describe --always --tags | set /P GIT_DESCRIBE_ALWAYS
//    //        - git describe --always --tags --long | set /P GIT_DESCRIBE_ALWAYS_LONG
//    //        - git describe --always --all --long | set /P GIT_DESCRIBE_AAL
//    //        - git describe --exact-match --tags | set /P GIT_OPTIONAL_TAG
//    //     git rev-parse --abbrev-ref HEAD | set /P GIT_OPTIONAL_BRANCH
//    // rustc -V
//    // cargo -V
//    // cd c_components && cargo info > file
//    //RUSTC version
//    //Gcc/clang version
//
//    CompilationInfo{
//        commands: c,
//        env_vars: env,
//        conan_info:
//    }
//}

pub fn write_version <P: AsRef<Path>>(topdir: P, file_contents: String) -> Result<(), Error> {
    let path = env::var_os("OUT_DIR").ok_or(Error::MissingEnvVar)?;
    let path : &Path = path.as_ref();
    create_dir_all(path)?;

    let path = path.join("version.rs");
    let mut file = BufWriter::new(File::create(&path)?);
    write!(file, "{}", file_contents)?;
    Ok(())
}



const PACKAGE_TOP_DIR : &'static str = ".";

fn main() {


//    let output = Command::new("/bin/cat")
//        .arg("file.txt")
//        .output()
//        .expect("failed to execute process");


    //Pull in imageflow_core/conaninfo.txt for compiler versions and dependencies


//    - git rev-parse --short HEAD | set /P GIT_COMMIT_SHORT
//        - git describe --always --tags | set /P GIT_DESCRIBE_ALWAYS
//        - git describe --always --tags --long | set /P GIT_DESCRIBE_ALWAYS_LONG
//        - git describe --always --all --long | set /P GIT_DESCRIBE_AAL
//        - git describe --exact-match --tags | set /P GIT_OPTIONAL_TAG
//     git rev-parse --abbrev-ref HEAD | set /P GIT_OPTIONAL_BRANCH
// rustc -V
    // cargo -V
    // cd c_components && cargo info > file
    //RUSTC version
    //Gcc/clang version
    //conan package versions?

    let mut contents = String::new();
    for name in ENV_VARS.iter(){
        if let Some(value) = env::var(name).ok(){
            let line = format!("static {}: &'static str = {:?};\n", name,value);
            contents += &line;
        }
    }

    write_version(PACKAGE_TOP_DIR, contents ).expect("Saving git version");
}