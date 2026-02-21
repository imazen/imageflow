#[macro_use]
extern crate quick_error;
extern crate chrono;
use chrono::*;
use std::collections::HashMap;
use std::convert::AsRef;
use std::env;
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Read, Write};
use std::path::Path;
use std::process::{Command, Output};
use std::time::Instant;
extern crate rayon;
use rayon::prelude::*;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Io(err: std::io::Error) {
            from()
        }
        MissingEnvVar {
        }
        CommandFailed(err: Output){
            from()
        }
        CommandEmptyOutput(err: String){
            from()
        }
    }
}

pub enum EnvTidbit {
    Env(&'static str),
    EnvReq(&'static str),
    Cmd { key: &'static str, cmd: &'static str },
    CmdReq { key: &'static str, cmd: &'static str },
    CmdOrEnvReq { key: &'static str, cmd: &'static str },
    CmdOrEnv { key: &'static str, cmd: &'static str },
    EnvOrCmdInconsistent { key: &'static str, cmd: &'static str },
    FileContentsReq { key: &'static str, relative_to_build_rs: &'static str },
}

fn run(cmd: &str) -> std::result::Result<String, Error> {
    let mut args: Vec<&str> = cmd.split(" ").collect::<Vec<&str>>();
    if args.is_empty() {
        panic!("");
    }
    let exe = args.remove(0);

    let start = Instant::now();
    let output: Output = Command::new(exe).args(&args).output()?;
    let duration = start.elapsed();

    if duration.as_millis() > 500 {
        println!("Warning: command `{}` took {}ms to execute", cmd, duration.as_millis());
    }

    if !output.status.success() {
        return Err(Error::CommandFailed(output));
    }
    let utf8_msg = format!("Command produced invalid UTF-8 output: {}", cmd);

    let str_out: &str = std::str::from_utf8(&output.stdout).expect(&utf8_msg);
    if str_out.split_whitespace().count() > 0 {
        Ok(str_out.trim().to_owned())
    } else {
        Err(Error::from(str_out.to_owned()))
    }
}
fn fetch_env(key: &str, result_required: bool, empty_is_missing: bool) -> Option<String> {
    if result_required {
        match env::var(key) {
            Ok(ref v) if v.is_empty() && empty_is_missing => {
                panic!(
                    "Required env var {} is present - but empty - in the build environment",
                    key
                );
            }
            Ok(v) => Some(v),
            Err(e) => {
                panic!("Required env var {} missing in the build environment: {:?}", key, e);
            }
        }
    } else {
        env::var(key)
            .ok()
            .and_then(|v| if v.is_empty() && empty_is_missing { None } else { Some(v) })
    }
}
fn command(key: &str, cmd: &str, result_required: bool, fallback_to_env: bool) -> Option<String> {
    //Panic only if non-UTF-8 output is sent
    let output = run(cmd);
    //Don't panic when fetching env var
    let env_val = match fallback_to_env {
        true => fetch_env(key, false, true),
        false => None,
    };

    //Ensure consistency if both are present
    if let Ok(ref out_str) = output {
        if let Some(ref env_str) = env_val {
            if out_str != env_str && out_str.trim() != env_str.trim() {
                panic!(
                    "Inconsistent values for {} and {}.\nCommand output: {}\nEnv var: {}",
                    key, cmd, out_str, env_str
                );
            }
        }
    }

    if result_required && output.is_err() && env_val.is_none() {
        if fallback_to_env {
            panic!("Failed to acquire {} (required for build). \nCommand {} resulted in {:?}, and ENV var {} was missing or empty.",
                   key, cmd, output, key);
        } else {
            panic!("Failed to acquire {} (required for build). \nCommand {} resulted in {:?}. ENV var not consulted.",
                   key, cmd, output);
        }
    } else {
        output.ok().or(env_val)
    }
}

fn env_or_cmd(key: &str, cmd: &str) -> Option<String> {
    fetch_env(key, false, true).or(run(cmd).ok())
}

//
//fn get_repo_root() -> PathBuf{
//    let build_rs_path = file!();
//    Path::new(&build_rs_path).parent().expect("Rust must be stripping parent directory info from file! macro. This breaks path stuff in build.rs.").to_owned()
//}

fn collect_info(shopping_list: Vec<EnvTidbit>) -> HashMap<String, Option<String>> {
    let results: Vec<(String, Option<String>)> = shopping_list
        .into_par_iter()
        .map(|from| {
            let (k, v) = match from {
                EnvTidbit::Env(key) => (key, fetch_env(key, false, true)),
                EnvTidbit::EnvReq(key) => (key, fetch_env(key, true, true)),
                EnvTidbit::FileContentsReq { key, relative_to_build_rs } => {
                    let io_error_expect = format!("Failed to read file {:?}. This file is required to be embedded in output binaries.", relative_to_build_rs);
                    let mut file = File::open(relative_to_build_rs).expect(&io_error_expect);
                    let mut contents = String::new();
                    file.read_to_string(&mut contents).expect(&io_error_expect);
                    (key, Some(contents))
                },
                EnvTidbit::Cmd { key, cmd } => (key, command(key, cmd, false, false)),
                EnvTidbit::CmdReq { key, cmd } => (key, command(key, cmd, true, false)),
                EnvTidbit::CmdOrEnvReq { key, cmd } => (key, command(key, cmd, true, true)),
                EnvTidbit::CmdOrEnv { key, cmd } => (key, command(key, cmd, false, true)),
                EnvTidbit::EnvOrCmdInconsistent { key, cmd } => (key, env_or_cmd(key, cmd)),
            };
            (k.to_owned(), v)
        })
        .collect();

    results.into_iter().collect()
}
fn what_to_collect() -> Vec<EnvTidbit> {
    let mut c = vec![
        EnvTidbit::CmdOrEnvReq { key: "GIT_COMMIT", cmd: "git rev-parse HEAD" },
        EnvTidbit::CmdOrEnv { key: "GIT_COMMIT_SHORT", cmd: "git rev-parse --short HEAD" },
        EnvTidbit::CmdOrEnv { key: "GIT_DESCRIBE_ALWAYS", cmd: "git describe --always --tags" },
        EnvTidbit::CmdOrEnvReq {
            key: "GIT_DESCRIBE_ALWAYS_LONG",
            cmd: "git describe --always --tags --long",
        },
        EnvTidbit::CmdOrEnv { key: "GIT_DESCRIBE_ALL", cmd: "git describe --always --all --long" },
        EnvTidbit::CmdOrEnv { key: "GIT_OPTIONAL_TAG", cmd: "git describe --exact-match --tags" },
        EnvTidbit::CmdOrEnv { key: "GIT_OPTIONAL_BRANCH", cmd: "git symbolic-ref --short HEAD" },
    ];

    static ENV_VARS: [&str; 22] = [
        "ESTIMATED_ARTIFACT_URL",
        "ESTIMATED_DOCS_URL",
        "CI_SEQUENTIAL_BUILD_NUMBER",
        "CI_BUILD_URL",
        "CI_JOB_URL",
        "CI_JOB_TITLE",
        "CI_STRING",
        "CI_PULL_REQUEST_INFO",
        "CI_TAG",
        "CI_RELEASE",
        "CI_REPO",
        "CI_RELATED_BRANCH",
        "CI",
        "TARGET",
        "OUT_DIR",
        "HOST",
        "OPT_LEVEL",
        "DEBUG",
        "PROFILE",
        "RUSTC",
        "RUSTFLAGS",
        "TARGET_CPU",
    ];
    for name in ENV_VARS.iter() {
        c.push(EnvTidbit::Env(name));
    }
    c.push(EnvTidbit::EnvReq("CARGO_MANIFEST_DIR"));
    c.push(EnvTidbit::Cmd { key: "GIT_STATUS", cmd: "git status" });
    c.push(EnvTidbit::Cmd { key: "GLIBC_VERSION", cmd: "ldd --version" });
    c.push(EnvTidbit::Cmd { key: "UNAME", cmd: "uname -av" });

    // only if CI_RELEASE==true
    if env::var("CI_RELEASE").unwrap_or("false".to_owned()).to_lowercase() == "true" {
        c.push(EnvTidbit::Cmd { key: "WIN_SYSTEMINFO", cmd: "systeminfo.exe" });
        // takes 3-9 seconds...
    }

    c.push(EnvTidbit::Cmd { key: "DEFAULT_GCC_VERSION", cmd: "gcc -v" });
    c.push(EnvTidbit::Cmd { key: "DEFAULT_CLANG_VERSION", cmd: "clang --version" });
    c.push(EnvTidbit::CmdReq { key: "DEFAULT_RUSTC_VERSION", cmd: "rustc -V" });
    c.push(EnvTidbit::CmdReq { key: "DEFAULT_CARGO_VERSION", cmd: "cargo -V" });
    c
}

fn write_file(name: &str, file_contents: String) -> std::result::Result<(), Error> {
    let path = env::var_os("OUT_DIR").ok_or(Error::MissingEnvVar)?;
    let path: &Path = path.as_ref();
    create_dir_all(path)?;

    let path = path.join(name);
    let mut file = BufWriter::new(File::create(&path)?);
    write!(file, "{}", file_contents)?;
    Ok(())
}

fn main() {
    let todo = what_to_collect();
    let utcnow_val = Utc::now();

    let mut results = collect_info(todo);
    results.insert("GENERATED_DATETIME_UTC".to_owned(), Some(utcnow_val.to_rfc3339()));
    results
        .insert("GENERATED_DATE_UTC".to_owned(), Some(utcnow_val.format("%Y-%m-%d").to_string()));

    let mut contents = String::new();
    contents += "use std::collections::HashMap;\n";
    // contents += "#[macro_use]\nextern crate lazy_static;\n";
    contents += "fn get_build_env_info() -> HashMap<&'static str, Option<&'static str>> {\n";
    contents += "  let mut i = HashMap::new();\n";
    for (k, v) in &results {
        let line = format!("  i.insert({:?}, {:?});\n", k, v);
        contents += &line;
    }
    contents += "  i\n}\nlazy_static! {\n  pub static ref BUILD_ENV_INFO: HashMap<&'static str, Option<&'static str>> = ";
    contents += "get_build_env_info();\n}\n";

    //These vars are required for all builds
    for name in [
        "GIT_COMMIT",
        "GIT_DESCRIBE_ALWAYS",
        "TARGET",
        "GENERATED_DATETIME_UTC",
        "GENERATED_DATE_UTC",
    ]
    .iter()
    {
        let value = results.get::<str>(name).unwrap().to_owned().unwrap();
        let line = format!("pub static {}: &str = {:?};\n", name, &value);
        contents += &line;
    }

    let ci_value =
        results.get("CI").unwrap().to_owned().unwrap_or("false".to_owned()).to_lowercase();
    let line = format!("pub static BUILT_ON_CI: bool = {};\n", ci_value);
    contents += &line;

    //    let line = format!("pub static GENERATED_DATETIME_UTC: &'static str = {:?};\n", utcnow_val.to_rfc3339());
    //    contents += &line;
    //    let line = format!("pub static GENERATED_DATE_UTC: &'static str = {:?};\n", utcnow_val.format("%Y-%m-%d").to_string());
    //    contents += &line;

    write_file("build_env_info.rs", contents).expect("Saving git version");
}
