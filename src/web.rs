use std::collections::HashMap;
use std::sync::Arc;

use warp::Filter;

use crate::query::{self, AppState, QueryError};

const FRONTEND_HTML: &str = include_str!("../static/index.html");
const API_JS: &str = include_str!("../static/api.js");

pub use query::{log_message, set_exe_dir_as_cwd};

pub async fn serve(
    host: std::net::IpAddr,
    port: u16,
    default_file: Option<String>,
    open_browser: bool,
) {
    let state = Arc::new(AppState::new(default_file));

    let (tx, rx) = std::sync::mpsc::channel();
    let port_u16 = port;
    let url = format!("http://{}:{}", host, port);
    if open_browser {
        std::thread::spawn(move || {
            rx.recv().ok();
            for _ in 0..20 {
                std::thread::sleep(std::time::Duration::from_millis(500));
                if std::net::TcpStream::connect((host, port_u16)).is_ok() {
                    open_browser_tab(&url);
                    break;
                }
            }
        });
    }

    let state_filter = warp::any().map(move || state.clone());

    let frontend = warp::path::end()
        .and(warp::get())
        .map(|| warp::reply::html(FRONTEND_HTML));

    let api_js = warp::path("api.js")
        .and(warp::get())
        .map(|| {
            warp::reply::with_header(API_JS, "content-type", "application/javascript; charset=utf-8")
        });

    let query_route = warp::path("api")
        .and(warp::path("query"))
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::query::<HashMap<String, String>>())
        .and(state_filter.clone())
        .and_then(handle_query)
        .boxed();

    let upload = warp::path("api")
        .and(warp::path("upload"))
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::query::<HashMap<String, String>>())
        .and(warp::body::content_length_limit(1024 * 1024 * 1024))
        .and(warp::body::bytes())
        .and(state_filter)
        .and_then(handle_upload)
        .boxed();

    let routes = frontend.or(api_js).or(query_route).or(upload);

    log_message(&format!("Starting server on {host}:{port}"));
    if open_browser {
        tx.send(()).ok();
    }
    warp::serve(routes).run((host, port)).await;
}

async fn handle_upload(
    params: HashMap<String, String>,
    body: bytes::Bytes,
    _state: Arc<AppState>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let name = params
        .get("name")
        .cloned()
        .unwrap_or_else(|| "upload.log".to_string());
    let temp_dir = std::env::temp_dir().join("logviewer");
    std::fs::create_dir_all(&temp_dir).ok();
    let file_path = temp_dir.join(format!(
        "{}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        name
    ));
    if std::fs::write(&file_path, &body[..]).is_err() {
        return Err(warp::reject::not_found());
    }
    let fp = file_path.clone();
    tokio::spawn(async move {
        tokio::time::delay_for(std::time::Duration::from_secs(3600)).await;
        std::fs::remove_file(fp).ok();
    });
    Ok(warp::reply::json(&serde_json::json!({
        "path": file_path.to_string_lossy(),
    })))
}

async fn handle_query(
    params: HashMap<String, String>,
    state: Arc<AppState>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match query::query_records(&state, &params) {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(QueryError::NotFound) => Err(warp::reject::not_found()),
    }
}

fn open_browser_tab(url: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
        // Bring browser to front so the user sees the tab
        let script = format!(
            r#"tell application "System Events" to set frontmost of first process whose name contains "Safari" or name contains "Chrome" or name contains "Firefox" or name contains "Edge" or name contains "Arc" to true"#
        );
        std::thread::sleep(std::time::Duration::from_millis(300));
        let _ = std::process::Command::new("osascript").args(["-e", &script]).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .ok();
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn().ok();
    }
}
