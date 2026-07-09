#![cfg_attr(all(windows, feature = "gui"), windows_subsystem = "windows")]

use clap::{crate_version, Arg, Command};
use std::fs::File;
use std::io::{Write, stdout};
#[cfg(not(feature = "web"))]
use std::process;

use logviewer::process;
use logviewer::readers;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Command::new("logviewer")
        .about("Log Viewer")
        .version(crate_version!())
        .author("Remi Rampin <remirampin@gmail.com>")
        .subcommand(
            Command::new("process")
                .about("Process a log file according to a view (JSON) and output records (JSON lines)")
                .arg(Arg::new("VIEW").required(true).help("View definition (JSON file)"))
                .arg(Arg::new("LOG").required(true).help("Log file")),
        );
    #[cfg(feature = "web")]
    let app = app.subcommand(
        Command::new("web")
            .about("Start a local webserver to analyze logs")
            .arg(Arg::new("LOG").required(false).help("Log file (optional - upload via UI)"))
            .arg(
                Arg::new("VIEW")
                    .long("view")
                    .short('v')
                    .help("View definition (JSON file)"),
            ),
    );

    let matches = app.get_matches();

    #[cfg(feature = "web")]
    let (command, matches) = match matches.subcommand() {
        Some((cmd, sub)) => (cmd, sub),
        None => ("web", &matches),
    };
    #[cfg(not(feature = "web"))]
    let (command, matches) = match matches.subcommand() {
        None => {
            eprintln!("No command specified.");
            process::exit(1);
        }
        Some((cmd, sub)) => (cmd, sub),
    };

    match command {
        "process" => {
            let reader = {
                let path = matches.get_one::<std::ffi::OsString>("LOG").unwrap();
                readers::detect_reader(path).unwrap_or_else(|_| {
                    Box::new(readers::LogFile::open(path).unwrap())
                })
            };
            let view = {
                let path = matches.get_one::<std::ffi::OsString>("VIEW").unwrap();
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

            // TODO: Allocate concrete colors for FromValue colors, print with
            // ANSI colors in terminal
        }
        #[cfg(feature = "web")]
        "web" => {
            logviewer::web::set_exe_dir_as_cwd();
            let log_path = matches
                .get_one::<std::ffi::OsString>("LOG")
                .map(|p| p.to_string_lossy().into_owned());
            if let Some(ref path) = log_path {
                logviewer::web::log_message(&format!("Log file: {path}"));
            } else {
                logviewer::web::log_message("No log file specified - load via UI upload.");
            }
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(logviewer::web::serve(
                [127, 0, 0, 1].into(),
                8000,
                log_path,
                true,
            ));
        }
        _ => panic!("Missing code for command {}", command),
    }
    Ok(())
}
