
use crate::build_env_info as benv;
use std;


pub fn dirty() -> bool {
    match *benv::BUILD_ENV_INFO.get("GIT_STATUS").unwrap() {
        Some(v) => v.contains("modified"),
        None => true, //because we don't know
    }
}

/// The parent folder to this crate
pub fn crate_parent_folder() -> std::path::PathBuf{
    ::std::path::Path::new(get_build_env_value("CARGO_MANIFEST_DIR").unwrap()).parent().unwrap().to_owned()
}


fn dirty_star() -> &'static str {
    if dirty() { "*" } else { "" }
}

pub fn last_commit() -> &'static str {
    benv::GIT_COMMIT
}


fn commit9_and_date() -> String {
    format!("{}{} {}",
            &benv::GIT_COMMIT[0..9],
            dirty_star(),
            benv::GENERATED_DATE_UTC)
}

fn one_line_suffix() -> String {
    let c9d = commit9_and_date();
    format!("({}) for {}", c9d, benv::TARGET)
}



pub fn get_build_env_value(key: &str) -> &Option<&'static str> {
    static NONE:Option<&'static str>  = None;
     match benv::BUILD_ENV_INFO.get(key){
         Some(v) => v,
         None => &NONE
     }
}
pub fn get_build_date() -> &'static str{
    benv::GENERATED_DATETIME_UTC
}

fn built_ago() -> (i64, &'static str){
    let compiled_utc = ::chrono::DateTime::parse_from_rfc3339(benv::GENERATED_DATETIME_UTC).unwrap();
    let duration = ::chrono::Utc::now().signed_duration_since(compiled_utc);
    let (v,u) = if duration.num_days() > 0 {
        (duration.num_days(), "days")
    }else if duration.num_hours() > 0{
        (duration.num_hours(), "hours")
    }else if duration.num_minutes() > 0{
        (duration.num_minutes(), "minutes")
    }else {
        (duration.num_seconds(), "seconds")
    };
    if v < 1{
        (v, &u[0..u.len()-1])
    }else{
        (v,u)
    }

}

pub fn one_line_version() -> String {
    //Create options for branch and release_tag
    let branch = benv::BUILD_ENV_INFO.get("GIT_OPTIONAL_BRANCH").unwrap(); //still needs to be unwrapped
    let release_tag = if let Some(tag) = *benv::BUILD_ENV_INFO.get("GIT_OPTIONAL_TAG").unwrap() {
        if tag.starts_with('v') {
            Some(&tag[1..])
        } else { None }
    } else {None};


    let profile_release = get_build_env_value("PROFILE") == &Some("release");
    let ci_job_title = get_build_env_value("CI_JOB_TITLE").unwrap_or("Local ");
    let profile = get_build_env_value("PROFILE").unwrap_or("[profile missing]");
    let ci = benv::BUILT_ON_CI;

    let target_cpu = match get_build_env_value("TARGET_CPU").unwrap_or(get_build_env_value("RUSTFLAGS").unwrap_or("?")){
        "x86-64" | "x86"=> "",
        "native" => "HOST NATIVE CPU (not portable)",
        other => other
    };


    if ci && profile_release && release_tag.is_some(){
        format!("release {} {} {}", release_tag.unwrap(), one_line_suffix(), target_cpu)
    }else if ci && profile_release && branch == &Some("master") && !dirty() {
        format!("nightly {} from master {} {}",
                benv::GIT_DESCRIBE_ALWAYS,
                one_line_suffix(), target_cpu)
    }else{

        let (v, unit) = built_ago();

        let source = if ci_job_title.starts_with("Travis 88888"){
            "simulation CI"
        }else if ci_job_title.starts_with("Travis ") ||  ci_job_title.starts_with("AppVeyor ") {
            "unofficial CI"
        }else if ci_job_title.starts_with("Local ") {
            "user-compiled"
        }else  {
            "SOURCE UNKNOWN"
        };


        format!("built {} {} ago - {} {} build of {}{} ({}) for {} {} ({})",
                v, unit, source, profile,
                benv::GIT_DESCRIBE_ALWAYS, dirty_star(), branch.as_ref().unwrap_or(&"unknown branch"), benv::TARGET, target_cpu, benv::GENERATED_DATE_UTC)
    }
}

pub fn all_build_info_pairs() -> String {
    // channel matters
    // tagged status matters

    let mut pairs: Vec<(&&'static str, &Option<&'static str>)> = benv::BUILD_ENV_INFO.iter()
        .collect();
    pairs.sort_by(|a, b| {
        let a_lines = match *a.1 {
            Some(text) => text.lines().count(),
            None => 0,
        };
        let b_lines = match *b.1 {
            Some(text) => text.lines().count(),
            None => 0,
        };
        if a_lines > 1 || b_lines > 1 {
            match a_lines.cmp(&b_lines) {
                std::cmp::Ordering::Equal => a.0.cmp(b.0),
                other => other,
            }
        } else {
            a.0.cmp(b.0)
        }
    });

    let mut s = String::new();

    for (k, v) in pairs {
        let line = match *v {
            Some(s) => format!("{}={}\n", k, s),
            None => format!("{} (None)\n", k),
        };
        s += &line;
    }
    s
}
