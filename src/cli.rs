use clap::{Arg, ArgMatches, Command};
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Write, stdout};

use logviewer::process;
use logviewer::readers;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Command::new("logviewer")
        .about("Log Viewer")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(
            Command::new("process")
                .about("Process a log file according to a view (JSON) and output records (JSON lines)")
                .arg(Arg::new("VIEW").help("View definition (JSON file)").required(true))
                .arg(Arg::new("LOG").help("Log file").required(true)),
        );
    #[cfg(feature = "web")]
    let app = app.subcommand(
        Command::new("web")
            .about("Start a local webserver to analyze logs")
            .arg(Arg::new("LOG").help("Log file (optional - upload via UI)"))
            .arg(
                Arg::new("VIEW")
                    .long("view")
                    .short('v')
                    .help("View definition (JSON file)"),
            ),
    );
    let matches = app.get_matches();

    let (cmd, sub) = matches
        .subcommand()
        .unwrap_or(("web", &matches));

    match cmd {
        "process" => run_process(sub),
        #[cfg(feature = "web")]
        "web" => run_web(sub),
        _ => {
            eprintln!("Unknown command: {cmd}");
            std::process::exit(1);
        }
    }
}

fn run_process(matches: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let view_path = matches.get_one::<String>("VIEW").unwrap();
    let log_path = matches.get_one::<String>("LOG").unwrap();

    let reader = {
        readers::detect_reader(OsStr::new(log_path)).unwrap_or_else(|_| {
            Box::new(readers::LogFile::open(OsStr::new(log_path)).unwrap())
        })
    };
    let view = {
        let file = File::open(view_path)?;
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
fn run_web(matches: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    logviewer::web::set_exe_dir_as_cwd();
    let log_path = matches
        .get_one::<String>("LOG")
        .map(|p| p.to_owned());
    if let Some(ref path) = log_path {
        logviewer::web::log_message(&format!("Log file: {path}"));
    } else {
        logviewer::web::log_message("No log file specified - load via UI upload.");
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
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
fn run_web(_: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Web feature not enabled. Rebuild with --features web");
    std::process::exit(1);
}


