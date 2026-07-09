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

    let open_file = warp::path("api")
        .and(warp::path("open"))
        .and(warp::path::end())
        .and(warp::get())
        .and_then(handle_open_file)
        .boxed();

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

    let routes = frontend.or(api_js).or(open_file).or(query_route).or(upload);

    eprintln!("Starting server on {}:{}", host, port);
    log_message(&format!("Starting server on {host}:{port}"));
    if open_browser {
        tx.send(()).ok();
    }
    warp::serve(routes).run((host, port)).await;
}

async fn handle_open_file() -> Result<impl warp::Reply, warp::Rejection> {
    log_message("api/open: showing file dialog");
    let picked = tokio::task::spawn_blocking(|| {
        rfd::FileDialog::new()
            .add_filter("Log files", &["out", "OUT", "log", "txt"])
            .set_title("Open log file")
            .pick_file()
    })
    .await
    .map_err(|_| warp::reject::not_found())?;

    let Some(path) = picked else {
        log_message("api/open: cancelled");
        return Ok(warp::reply::json(&serde_json::json!({ "path": null })));
    };

    let path_str = path.to_string_lossy().into_owned();
    log_message(&format!("api/open: selected {path_str}"));
    Ok(warp::reply::json(&serde_json::json!({ "path": path_str })))
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
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
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
        std::process::Command::new("open").arg(url).spawn().ok();
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
