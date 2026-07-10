use clap::{crate_version, App, Arg};
use std::fs::File;
use std::io::{Write, stdout};

use logviewer::process;
use logviewer::readers;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = App::new("logviewer")
        .about("Log Viewer")
        .version(crate_version!())
        .author("Remi Rampin <remirampin@gmail.com>")
        .subcommand(
            App::new("process")
                .about("Process a log file according to a view (JSON) and output records (JSON lines)")
                .arg(Arg::with_name("VIEW").required(true).help("View definition (JSON file)"))
                .arg(Arg::with_name("LOG").required(true).help("Log file")),
        );
    #[cfg(feature = "web")]
    let app = app.subcommand(
        App::new("web")
            .about("Start a local webserver to analyze logs")
            .arg(Arg::with_name("LOG").required(false).help("Log file (optional - upload via UI)"))
            .arg(
                Arg::with_name("VIEW")
                    .long("view")
                    .short("v")
                    .help("View definition (JSON file)"),
            ),
    );
    #[cfg(feature = "desktop")]
    let app = app.subcommand(
        App::new("desktop")
            .about("Launch native desktop application")
            .arg(Arg::with_name("LOG").required(false).help("Log file (optional - upload via UI)")),
    );

    let matches = app.get_matches();

    #[cfg(feature = "desktop")]
    let default_cmd = "desktop";
    #[cfg(not(feature = "desktop"))]
    let default_cmd = "web";

    let cmd = matches.subcommand_name().unwrap_or(default_cmd);
    let sub = matches.subcommand_matches(cmd).unwrap_or(&matches);

    match cmd {
        "process" => run_process(sub),
        #[cfg(feature = "web")]
        "web" => run_web(sub),
        #[cfg(feature = "desktop")]
        "desktop" => run_desktop(sub),
        _ => {
            eprintln!("Unknown command: {cmd}");
            std::process::exit(1);
        }
    }
}

fn run_process(matches: &clap::ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let reader = {
        let path = matches.value_of_os("LOG").unwrap();
        readers::detect_reader(path).unwrap_or_else(|_| {
            Box::new(readers::LogFile::open(path).unwrap())
        })
    };
    let view = {
        let path = matches.value_of_os("VIEW").unwrap();
        let file = File::open(path)?;
        serde_json::from_reader(file)?
    };

    let out = stdout();
    let mut out = out.lock();
    for record in process(reader, view) {
        let record = record?;
        serde_json::to_writer(&mut out, &record)?;
        writeln!(out)?;
    }
    Ok(())
}

#[cfg(feature = "web")]
fn run_web(matches: &clap::ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    logviewer::web::set_exe_dir_as_cwd();
    let log_path = matches
        .value_of_os("LOG")
        .map(|p| p.to_string_lossy().into_owned());
    if let Some(ref path) = log_path {
        logviewer::web::log_message(&format!("Log file: {path}"));
    } else {
        logviewer::web::log_message("No log file specified - load via UI upload.");
    }
    let mut runtime = tokio::runtime::Builder::new()
        .basic_scheduler()
        .enable_all()
        .build()?;
    runtime.block_on(logviewer::web::serve(
        [127, 0, 0, 1].into(),
        8000,
        log_path,
        true,
    ));
    Ok(())
}

#[cfg(not(feature = "web"))]
fn run_web(_: &clap::ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Web feature not enabled. Rebuild with --features web");
    std::process::exit(1);
}

#[cfg(feature = "desktop")]
fn run_desktop(matches: &clap::ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let log_path = matches
        .value_of_os("LOG")
        .map(|p| p.to_string_lossy().into_owned());
    logviewer::desktop::run(log_path);
    Ok(())
}

#[cfg(not(feature = "desktop"))]
fn run_desktop(_: &clap::ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Desktop feature not enabled. Rebuild with --features desktop");
    std::process::exit(1);
}
