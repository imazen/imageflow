use std;
use build_env_info as benv;


fn dirty() -> bool{
    match *benv::BUILD_ENV_INFO.get("GIT_STATUS").unwrap(){
        Some(v) => v.contains("modified"),
        None => true //because we don't know
    }
}

fn dirty_star() -> &'static str{
    if dirty(){
        "*"
    }else{ ""}
}

fn describe_always_dirty() -> String{
    if dirty(){
        format!("{}*",benv::GIT_DESCRIBE_ALWAYS)
    }else{
        format!("{}",benv::GIT_DESCRIBE_ALWAYS)
    }
}
fn commit9_and_date() -> String{
    format!("{}{} {}", &benv::GIT_COMMIT[0..9], dirty_star(), benv::GENERATED_DATE_UTC)
}

fn one_line_suffix() -> String{
    let c9d = commit9_and_date();
    format!("({}) for {}", c9d, benv::TARGET)
}

pub fn one_line_version() -> String{
    let branch = benv::BUILD_ENV_INFO.get("GIT_OPTIONAL_BRANCH").unwrap();
    match benv::BUILT_ON_CI{
        false => {
            format!("unofficial build of {} {}", describe_always_dirty(), one_line_suffix() )
        }
        true => {
            match benv::BUILD_ENV_INFO.get("GIT_OPTIONAL_TAG").unwrap(){
                &Some(ref tag) if tag.starts_with("v") => {
                    format!("release {} {}", &tag[1..], one_line_suffix() )
                }
                _  => {
                    if let Some(ref branch_name) = *branch {
                        format!("nightly build {} from branch {} {}", describe_always_dirty(), branch_name, one_line_suffix())
                    } else {
                        format!("unknown build {} {} ", describe_always_dirty(), one_line_suffix())
                    }
                }
            }
        }
    }
}

pub fn all_build_info_pairs() -> String{
    //channel matters
    //tagged status matters

    let mut pairs: Vec<(&&'static str, &Option<&'static str>)> = benv::BUILD_ENV_INFO.iter().collect();
    pairs.sort_by(|a, b| {
        let a_lines = match *a.1{
            Some(text) => text.lines().count(),
            None => 0
        };
        let b_lines = match *b.1 {
            Some(text) => text.lines().count(),
            None => 0
        };
        if a_lines > 1 || b_lines > 1{
            match a_lines.cmp(&b_lines){
                std::cmp::Ordering::Equal => a.0.cmp(b.0),
                other => other
            }
        }else {
            a.0.cmp(b.0)
        }
    });

    let mut s = String::new();

    for (k, v) in pairs{
        let line = match *v {
            Some(ref s) => format!("{}={}\n", k, s),
            None => format!("{} (None)\n", k)
        };
        s += &line;
    }
    s
}