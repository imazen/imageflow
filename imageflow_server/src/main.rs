extern crate clap;
extern crate imageflow_server;
#[macro_use] extern crate lazy_static;
extern crate regex;

use regex::Regex;

use clap::{App, Arg, SubCommand};
use ::imageflow_server::preludes::*;
use ::std::path::{Path, PathBuf};

use std::net::ToSocketAddrs;

extern crate imageflow_types as s;

fn main() {
    let exit_code = main_with_exit_code();
    std::process::exit(exit_code);
}


fn parse_mount(s: &str) -> std::result::Result<MountLocation, String>{
    //.split permits empty elements. We just join consecutive empty elements to allow escaping of : via ::
    let mut parts = s.split(":").fold((Vec::new(),false), | (mut list, previous_empty), item| {
        if previous_empty && item.is_empty(){
            (list, false)
        }else if item.is_empty(){
            list.push(item.to_owned());
            (list, true)
        }else{
            list.push(item.to_owned());
            (list, false)
        }
    }).0;

    if parts.len() < 2 {
        Err(format!("--mount prefix:engine:args  Mount value must contain at least prefix:engine - received {:?} ({:?})", s, &parts))
    }else{
        MountLocation::parse(parts.remove(0), parts.remove(0), parts)
    }
}

fn main_with_exit_code() -> i32 {
    let version = s::version::one_line_version();
    let app = App::new("imageflow_server").version(version.as_ref())
        .arg(Arg::with_name("port").long("port").takes_value(true).required(false).help("Change the port that the server will listen on"))
        .subcommand(
            SubCommand::with_name("diagnose")
                .about("Diagnostic utilities")
                .arg(
                    Arg::with_name("show-compilation-info").long("show-compilation-info")
                        .help("Show all the information stored in this executable about the environment in which it was compiled.")
                ).arg(
                Arg::with_name("call-panic").long("call-panic")
                    .help("Triggers a Rust panic (so you can observe failure/backtrace behavior)")
            )
        )
        .subcommand(
            SubCommand::with_name("start")
                .about("Start server")
                                .arg(
                                    Arg::with_name("mount").long("mount").takes_value(true).empty_values(false).multiple(true).min_values(1).validator(|f| parse_mount(&f).map(|r| ()))
                                        .help("Serve images from the given location using the provided API, e.g --mount \"/prefix/:ir4_local:./{}\" --mount \"/extern/:ir4_http:https:://domain.com/{}\"\n Escape colons by doubling, e.g. http:// -> http:://")
                                )
            .arg(Arg::with_name("bind").long("bind").takes_value(true).required(true).validator(|s| {
             s.to_socket_addrs()  .ok().and_then(|mut addrs| addrs.next()).map(|v| ()).ok_or("".to_owned())})
                .help("The socket to bind to, like localhost:80 or ::1:80, (to make public on all addresses, use 0.0.0.0:80. Better if you reverse proxy for now, and only bind to localhost)"
    )).arg(Arg::with_name("data-dir").long("data-dir").takes_value(true).required(true).validator(|f| if Path::new(&f).is_dir() { Ok(()) } else { Err(format!("The specified data-dir {} must be an existing directory.", f))} )
            .help("An existing directory for logging and caching"))
        )
        .subcommand(
            SubCommand::with_name("demo")
                .about("Start demo server on localhost:39876 with mounts /ir4/proxy/unsplash -> http://images.unsplash.com/")
                .arg(Arg::with_name("data-dir").long("data-dir").takes_value(true).required(true).validator(|f| if Path::new(&f).is_dir() { Ok(()) } else { Err(format!("The specified data-dir {} must be an existing directory.", f))} )
                .help("An existing directory to be used for logging and caching"))
        )
    ;



    let matches = app.get_matches();

    let port = matches.value_of("port").map(|s| s.parse::<u16>().unwrap() );

    if let Some(ref matches) = matches.subcommand_matches("diagnose") {
        let m: &&clap::ArgMatches = matches;

        if m.is_present("show-compilation-info") {
            println!("{}\n{}\n",
                     s::version::one_line_version(),
                     s::version::all_build_info_pairs());
            return 0;
        }
        if m.is_present("call-panic") {
            panic!("Panicking on command");
        }
    }
    if let Some(ref matches) = matches.subcommand_matches("start") {
        let m: &&clap::ArgMatches = matches;

        let data_dir = m.value_of("data-dir").map(|s| PathBuf::from(s)).expect("data-dir required");
        let bind = m.value_of("bind").map(|s| s.to_owned()).expect("bind address required");

        let mounts = m.values_of_lossy("mount").expect("at least one --mount required").into_iter().map(|s| parse_mount(&s).expect("validator not working - bug in clap?")).collect::<Vec<MountLocation>>();


        ::imageflow_server::serve(StartServerConfig{
            bind_addr: bind,
            data_dir: data_dir,
            mounts: mounts,
            default_cache_layout: Some(FolderLayout::Normal),
        });
        return 0;
    }
    if let Some(ref matches) = matches.subcommand_matches("demo") {
        let m: &&clap::ArgMatches = matches;

        let data_dir = m.value_of("data-dir").map(|s| PathBuf::from(s)).expect("data-dir required");

        //TODO: fetch an examples directory, with javascript/html/css and images, and mount that

        let bind = format!("localhost:{}", port.unwrap_or(39876));
        ::imageflow_server::serve(StartServerConfig {
            bind_addr: bind,
            data_dir: data_dir,
            default_cache_layout: Some(FolderLayout::Tiny),
            mounts: vec![
            MountLocation {
                engine: MountedEngine::Ir4Http,
                prefix: "/ir4/proxy_unsplash/".to_owned(),
                engine_args: vec!["http://images.unsplash.com/".to_owned()]
            }
            ]
        });
        return 0;
    }
    64
}

