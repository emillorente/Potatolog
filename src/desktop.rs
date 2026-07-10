use std::sync::mpsc;
use tao::event::WindowEvent;
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

use crate::query;
use crate::web;

fn find_port() -> u16 {
    for port in 8000..9000 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
            return port;
        }
    }
    8000
}

pub fn run(log_path: Option<String>) {
    std::panic::set_hook(Box::new(|info| {
        let msg = info.payload().downcast_ref::<&str>().map(|s| *s)
            .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or("unknown panic");
        let loc = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        query::log_message(&format!("PANIC {msg} at {loc}"));
    }));
    query::set_exe_dir_as_cwd();
    let port = find_port();
    let url = format!("http://127.0.0.1:{port}");

    if let Some(ref path) = log_path {
        query::log_message(&format!("Log file: {path}"));
    } else {
        query::log_message("No log file specified - load via UI upload.");
    }

    let (ready_tx, ready_rx) = mpsc::channel();
    let server_log = log_path.clone();
    let ready_tx_thread = ready_tx.clone();

    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(|| {
            let mut runtime = tokio::runtime::Builder::new()
                .threaded_scheduler()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(web::serve(
                [127, 0, 0, 1].into(),
                port,
                server_log,
                false,
            ));
        });
        match result {
            Ok(()) => query::log_message("Server exited normally"),
            Err(panic) => {
                let msg = if let Some(s) = panic.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                query::log_message(&format!("Server thread panicked: {msg}"));
                let _ = ready_tx_thread.send(Err(msg));
            }
        }
    });

    // Wait for server to be ready (probe the port)
    for _ in 0..10 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            let _ = ready_tx.send(Ok(()));
            break;
        }
        if let Ok(Err(_)) = ready_rx.try_recv() {
            query::log_message("Server failed, exiting");
            std::process::exit(1);
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }

    if let Ok(Err(e)) = ready_rx.try_recv() {
        query::log_message(&format!("Server failed: {e}"));
        std::process::exit(1);
    }

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("LogViewer")
        .with_inner_size(tao::dpi::LogicalSize::new(1280.0, 800.0))
        .build(&event_loop)
        .expect("Failed to create window");

    let _webview = WebViewBuilder::new()
        .with_url(&url)
        .build(&window)
        .expect("Failed to create WebView");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let tao::event::Event::WindowEvent { event: WindowEvent::CloseRequested, .. } = event {
            query::log_message("CloseRequested, exiting");
            std::process::exit(0);
        }
    });
}
