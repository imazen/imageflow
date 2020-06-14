extern crate clap;
extern crate imageflow_server;


use clap::{App, Arg, SubCommand, AppSettings};
use imageflow_server::preludes::*;
use std::path::{Path, PathBuf};

use std::net::ToSocketAddrs;

extern crate imageflow_types as s;

fn main() {
    let exit_code = main_with_exit_code();
    std::process::exit(exit_code);
}


fn parse_mount(s: &str) -> std::result::Result<MountLocation, String>{
    //Escape ::
    let mut parts = s.replace("::","||||||").split(':').map(|s| s.replace("||||||",":")).collect::<Vec<String>>();
    if parts.len() < 2 {
        Err(format!("--mount prefix:engine:args  Mount value must contain at least prefix:engine - received {:?} ({:?})", s, &parts))
    }else{
        MountLocation::parse(parts.remove(0), parts.remove(0), parts)
    }
}

fn main_with_exit_code() -> i32 {
    let version = s::version::one_line_version();
    let app = App::new("imageflow_server").version(version.as_ref())
        .setting(AppSettings::VersionlessSubcommands).setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("diagnose").setting(AppSettings::ArgRequiredElseHelp)
                .about("Diagnostic utilities")
                .arg(
                    Arg::with_name("show-compilation-info").long("show-compilation-info")
                        .help("Show all the information stored in this executable about the environment in which it was compiled.")
                ).arg(
                Arg::with_name("call-panic").long("call-panic")
                    .help("Triggers a Rust panic (so you can observe failure/backtrace behavior)")
            ).arg(
                Arg::with_name("smoke-test-core").long("smoke-test-core")
                    .help("Smoke test a few tiny image processing operations"))
        )
        .subcommand(
            SubCommand::with_name("start")
                .about("Start HTTP server").setting(AppSettings::ArgRequiredElseHelp)
                .arg(Arg::with_name("demo").long("demo").conflicts_with("mount").required_unless("mount")
                .help("Start demo server (on localhost:39876 by default) with mounts /ir4/proxy/unsplash -> http://images.unsplash.com/"))
                .arg(
                    Arg::with_name("mount").long("mount").takes_value(true).empty_values(false).multiple(true).required_unless("demo")
                        .validator(|f| parse_mount(&f).map(|_| ()))
                        .help("Serve images from the given location using the provided API, e.g --mount \"/prefix/:ir4_local:./{}\" --mount \"/extern/:ir4_http:http:://domain.com/\" --mount \"/extern/:ir4_proxy_uncached:http:://domain.com/\"\n Escape colons by doubling, e.g. http:// -> http:://")
                )
                .arg(Arg::with_name("bind-address").long("bind-address").takes_value(true).required(false).default_value("localhost")
                    .help("The IPv4 or IPv6 address to bind to (or the hostname, like localhost). 0.0.0.0 binds to all addresses."
                ))
                .arg(Arg::with_name("port").long("port").short("-p").takes_value(true).default_value("39876").required(false).help("Set the port that the server will listen on"))
                .arg(Arg::with_name("cert").long("certificate").takes_value(true).required(false).help("Path to a valid PKCS12 certificate (enables https)"))
                .arg(Arg::with_name("cert-pwd").long("certificate-password").takes_value(true).required(false).help("Password to the PKCS12 certificate"))


                .arg(Arg::with_name("data-dir").long("data-dir").takes_value(true).required_unless("demo")
                    .validator(|f| if Path::new(&f).is_dir() { Ok(()) } else { Err(format!("The specified data-dir {} must be an existing directory. ", f)) })
                .help("An existing directory for logging and caching"))
                .arg(Arg::with_name("integration-test").long("integration-test").hidden(true).help("Never use this outside of an integration test. Exposes an HTTP endpoint to kill the server."))


        );



    let matches = app.get_matches();

    if let Some(matches) = matches.subcommand_matches("diagnose") {
        let m: &clap::ArgMatches = matches;

        if m.is_present("show-compilation-info") {
            println!("{}\n{}\n",
                     s::version::one_line_version(),
                     s::version::all_build_info_pairs());
            return 0;
        }
        if m.is_present("call-panic") {
            panic!("Panicking on command");
        }
        if m.is_present("smoke-test-core") {
            ::imageflow_server::diagnose::smoke_test_core();
            return 0;
        }
    }
    if let Some(matches) = matches.subcommand_matches("start") {
        let m: &clap::ArgMatches = matches;


        let port = matches.value_of("port").map(|s| s.parse::<u16>().expect("Port must be a valid 16-bit positive integer") ).unwrap_or(39_876);
        let integration_test = matches.is_present("integration-test");
        let data_dir = m.value_of("data-dir").map(PathBuf::from);
        let cert = m.value_of("cert").map(PathBuf::from);
        if let Some(ref p) = cert{
            if !p.is_file(){
                println!("The provided certificate file does not exist: {:?}", &cert);
                std::process::exit(64);
            }
        }
        let bind = m.value_of("bind-address").map(|s| s.to_owned()).expect("bind address required");

        let combined = format!("{}:{}", bind, port);

        {
            let socket_addr_iter = combined.to_socket_addrs();
            if socket_addr_iter.is_err() || socket_addr_iter.unwrap().next().is_none() {
                println!("Invalid value for --bind-address. {} failed to parse.", &combined);
                std::process::exit(64);
            }
        }

        if m.is_present("demo"){
            //TODO: fetch an examples directory, with javascript/html/css and images, and mount that


            // If not provided, ./imageflow_data is created and used

            let alt_data_dir = Path::new(".").join("imageflow_data");


            let demo_commit = s::version::get_build_env_value("GIT_COMMIT").unwrap();

            let mut mounts = vec![
            MountLocation {
                engine: MountedEngine::Ir4Http,
                prefix: "/ir4/proxy_unsplash/".to_owned(),
                engine_args: vec!["http://images.unsplash.com/".to_owned()]
            },
            MountLocation {
                engine: MountedEngine::PermacacheProxyGuessContentTypes,
                prefix: "/proxied_demo/".to_owned(),
                engine_args: vec![format!("https://raw.githubusercontent.com/imazen/imageflow/{}/imageflow_server/demo/", demo_commit)]
            },
            MountLocation {
                engine: MountedEngine::Ir4Http,
                prefix: "/demo_images/".to_owned(),
                engine_args: vec!["http://resizer-images.s3.amazonaws.com/".to_owned()]
            },
            MountLocation {
                engine: MountedEngine::Ir4Http,
                prefix: "/website_images/".to_owned(),
                engine_args: vec!["http://resizer-web.s3.amazonaws.com/".to_owned()]
            },
            MountLocation {
                engine: MountedEngine::Ir4ProxyUncached,
                prefix: "/demo_images_uncached/".to_owned(),
                engine_args: vec!["http://resizer-images.s3.amazonaws.com/".to_owned()]
            }
            ];
            let local_demo_folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("demo");
            if local_demo_folder.exists() {
                mounts.push(MountLocation {
                                    engine: MountedEngine::Static,
                                    prefix: "/src_demo/".to_owned(),
                                    engine_args: vec![local_demo_folder.as_path().to_str().unwrap().to_owned()]
                                });

                println!("Open your browser to http://{}/src_demo/index.html", &combined);
            }else{
                println!("Open your browser to http://{}/proxied_demo/index.html", &combined);

            }

            println!("{}",&version);
            ::imageflow_server::serve(StartServerConfig {
                bind_addr: combined,
                data_dir: data_dir.unwrap_or_else(|| { if !alt_data_dir.exists() { std::fs::create_dir_all(&alt_data_dir).unwrap(); } alt_data_dir }),
                default_cache_layout: Some(FolderLayout::Tiny),
                integration_test: integration_test,
                mounts: mounts,
                cert: cert,
                cert_pwd: m.value_of("cert-pwd").map(|s| s.into()),
            });
        }else {
            let mounts = m.values_of_lossy("mount").expect("at least one --mount required").into_iter().map(|s| parse_mount(&s).expect("validator not working - bug in clap?")).collect::<Vec<MountLocation>>();

            println!("{}",&version);
            ::imageflow_server::serve(StartServerConfig {
                bind_addr: combined,
                data_dir: data_dir.expect("data-dir required"),
                mounts: mounts,
                default_cache_layout: Some(FolderLayout::Normal),
                integration_test: integration_test,
                cert: cert,
                cert_pwd: m.value_of("cert-pwd").map(|s| s.into()),
            });
        }
        return 0;
    }

    64
}

