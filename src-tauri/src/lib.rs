use std::net::{IpAddr, Ipv4Addr, TcpStream};
use std::thread;
use std::time::Duration;

use logviewer::query;
use logviewer::web;

const DESKTOP_PORT: u16 = 8731;

fn wait_for_server(port: u16) -> bool {
    for _ in 0..100 {
        if TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_ok() {
            return true;
        }
        thread::sleep(Duration::from_millis(100));
    }
    false
}

fn start_embedded_server() {
    thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.block_on(web::serve(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            DESKTOP_PORT,
            None,
            false,
        ));
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    query::set_exe_dir_as_cwd();
    query::log_message("Starting LogViewer desktop");

    start_embedded_server();

    if wait_for_server(DESKTOP_PORT) {
        query::log_message(&format!(
            "Embedded server ready on http://127.0.0.1:{DESKTOP_PORT}"
        ));
    } else {
        query::log_message(&format!(
            "Embedded server failed to start on port {DESKTOP_PORT}"
        ));
    }

    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
