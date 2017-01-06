
use build_env_info as benv;
use std;


fn dirty() -> bool {
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

fn describe_always_dirty() -> String {
    if dirty() {
        format!("{}*", benv::GIT_DESCRIBE_ALWAYS)
    } else {
        format!("{}", benv::GIT_DESCRIBE_ALWAYS)
    }
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

fn built_ago() -> (i64, &'static str){
    let compiled_utc = ::chrono::datetime::DateTime::parse_from_rfc3339(benv::GENERATED_DATETIME_UTC).unwrap();
    let duration = ::chrono::UTC::now() -compiled_utc;
    if duration.num_hours() > 0{
        (duration.num_hours(), "hours")
    }else if duration.num_minutes() > 0{
        (duration.num_minutes(), "minutes")
    }else {
        (duration.num_seconds(), "seconds")
    }
}

pub fn one_line_version() -> String {
    let branch = benv::BUILD_ENV_INFO.get("GIT_OPTIONAL_BRANCH").unwrap();
    match benv::BUILT_ON_CI {
        false => {
            let (v, unit) = built_ago();
            format!("built {} {} ago - UNOFFICIAL {} build of {}{} ({}) for {} ({})",
                   v,unit, get_build_env_value("PROFILE").unwrap_or("[profile missing]"),
                    benv::GIT_DESCRIBE_ALWAYS, dirty_star(), branch.as_ref().unwrap_or(&"unknown branch"), benv::TARGET, benv::GENERATED_DATE_UTC)

        }
        true => {
            match benv::BUILD_ENV_INFO.get("GIT_OPTIONAL_TAG").unwrap() {
                &Some(ref tag) if tag.starts_with("v") => {
                    format!("release {} {}", &tag[1..], one_line_suffix())
                }
                _ => {
                    if let Some(ref branch_name) = *branch {
                        format!("nightly build {} from branch {} {}",
                                describe_always_dirty(),
                                branch_name,
                                one_line_suffix())
                    } else {
                        format!("unknown build {} {} ",
                                describe_always_dirty(),
                                one_line_suffix())
                    }
                }
            }
        }
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
            Some(ref s) => format!("{}={}\n", k, s),
            None => format!("{} (None)\n", k),
        };
        s += &line;
    }
    s
}
