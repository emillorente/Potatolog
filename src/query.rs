use std::collections::HashMap;
use std::fs::File;
use std::sync::{Arc, RwLock};

use rayon::prelude::*;

use crate::filters::View;
use crate::process;
use crate::readers;

type RecordCache = Arc<RwLock<HashMap<String, Arc<CachedDataSet>>>>;

#[derive(Clone)]
struct CachedDataSet {
    records: Vec<crate::Record>,
    iso_dates: Vec<Option<String>>,
}

#[derive(Debug)]
pub enum QueryError {
    NotFound,
}

#[derive(Clone)]
pub struct AppState {
    pub default_file: Option<String>,
    cache: RecordCache,
}

impl AppState {
    pub fn new(default_file: Option<String>) -> Self {
        Self {
            default_file,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[derive(serde_derive::Serialize)]
pub struct QueryResponse {
    pub r: Vec<serde_json::Value>,
    pub t: usize,
}

pub fn set_exe_dir_as_cwd() {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let _ = std::env::set_current_dir(dir);
        }
    }
}

pub fn log_message(msg: &str) {
    eprintln!("{msg}");
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let log_path = dir.join("logviewer.log");
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
            {
                let _ = writeln!(file, "{msg}");
            }
        }
    }
}

pub fn query_records(state: &AppState, params: &HashMap<String, String>) -> Result<QueryResponse, QueryError> {
    let path = params
        .get("file")
        .cloned()
        .or_else(|| state.default_file.clone());
    let path = match path {
        Some(p) => p,
        None => {
            return Ok(QueryResponse {
                r: vec![],
                t: 0,
            });
        }
    };
    let refresh = params.get("refresh").map(|v| v == "true").unwrap_or(false);
    let data = cached_records(state, &path, refresh)?;
    let all_records = &data.records;
    let iso_dates = &data.iso_dates;
    let empty_ts = iso_dates.iter().filter(|d| d.is_none()).count();
    let skip: usize = params.get("skip").and_then(|v| v.parse().ok()).unwrap_or(0);
    let limit: usize = params.get("limit").and_then(|v| v.parse().ok()).unwrap_or(2000);
    let f_level = params.get("level").map(String::as_str).unwrap_or("");
    let f_comp = params.get("comp").map(String::as_str).unwrap_or("");
    let f_proc = params.get("proc").map(String::as_str).unwrap_or("");
    let f_thread = params.get("thread").map(String::as_str).unwrap_or("");
    let f_users = parse_user_filters(params.get("user").map(String::as_str).unwrap_or(""));
    let f_msg = params.get("msg").map(String::as_str).unwrap_or("");
    let f_source = params.get("source").map(String::as_str).unwrap_or("");
    let f_line = params.get("line").map(String::as_str).unwrap_or("");
    let f_source_c = params.get("sourceC").map(String::as_str).unwrap_or("");
    let f_line_c = params.get("lineC").map(String::as_str).unwrap_or("");
    let f_date_from = params.get("dateFrom").map(String::as_str).unwrap_or("");
    let f_date_to = params.get("dateTo").map(String::as_str).unwrap_or("");
    let f_date_to_adj: String;
    let f_date_to_ref = if !f_date_to.is_empty() && !f_date_to.contains(':') {
        f_date_to_adj = format!("{f_date_to}T23:59:59");
        f_date_to_adj.as_str()
    } else if !f_date_to.is_empty() && f_date_to.len() <= 16 {
        f_date_to_adj = format!("{f_date_to}:59");
        f_date_to_adj.as_str()
    } else {
        f_date_to
    };
    let f_hide_triggers = params.get("hideTriggers").map(|v| v == "true").unwrap_or(false);
    let search_q = params.get("q").map(String::as_str).unwrap_or("");

    let has_date_from = !f_date_from.is_empty();
    let has_date_to = !f_date_to.is_empty();
    let main_end = all_records.len() - empty_ts;
    let date_break = if has_date_from {
        let mut e = main_end;
        for i in 0..main_end {
            if let Some(ref iso) = iso_dates[i] {
                if iso.as_str() < f_date_from {
                    e = i;
                    break;
                }
            }
        }
        e
    } else {
        main_end
    };

    let matching: Vec<usize> = all_records[..date_break]
        .par_iter()
        .enumerate()
        .filter(|(i, rec)| {
            record_matches(
                rec,
                *i,
                main_end,
                iso_dates,
                f_level,
                f_comp,
                f_proc,
                f_thread,
                &f_users,
                f_msg,
                f_source,
                f_line,
                f_source_c,
                f_line_c,
                f_hide_triggers,
                has_date_to,
                f_date_to_ref,
                search_q,
            )
        })
        .map(|(i, _)| i)
        .chain((main_end..all_records.len()).into_par_iter().filter(|&i| {
            record_matches(
                &all_records[i],
                i,
                main_end,
                iso_dates,
                f_level,
                f_comp,
                f_proc,
                f_thread,
                &f_users,
                f_msg,
                f_source,
                f_line,
                f_source_c,
                f_line_c,
                f_hide_triggers,
                false,
                f_date_to_ref,
                search_q,
            )
        }))
        .collect();

    let total = matching.len();
    let out_records: Vec<serde_json::Value> = matching
        .iter()
        .skip(skip)
        .take(limit)
        .map(|&i| record_to_json(&all_records[i]))
        .collect();

    Ok(QueryResponse {
        r: out_records,
        t: total,
    })
}

#[allow(clippy::too_many_arguments)]
fn record_matches(
    rec: &crate::Record,
    i: usize,
    main_end: usize,
    iso_dates: &[Option<String>],
    f_level: &str,
    f_comp: &str,
    f_proc: &str,
    f_thread: &str,
    f_users: &[String],
    f_msg: &str,
    f_source: &str,
    f_line: &str,
    f_source_c: &str,
    f_line_c: &str,
    f_hide_triggers: bool,
    has_date_to: bool,
    f_date_to_ref: &str,
    search_q: &str,
) -> bool {
    let level = rec.variables.get("level").map_or("", |s| s);
    let comp = rec.variables.get("component").map_or("", |s| s);
    let proc = rec.variables.get("proc").map_or("", |s| s);
    let thread = rec.variables.get("thread").map_or("", |s| s);
    let user = rec.variables.get("user").map_or("", |s| s);
    let msg = rec.variables.get("message").map_or(&rec.text, |s| s);
    let source = rec.variables.get("source").map_or("", |s| s);
    let line = rec.variables.get("line").map_or("", |s| s);
    let source_c = rec.variables.get("sourceC").map_or("", |s| s);
    let line_c = rec.variables.get("lineC").map_or("", |s| s);

    if !f_level.is_empty() && !level.to_lowercase().contains(&f_level.to_lowercase()) {
        return false;
    }
    if !f_comp.is_empty() && !comp.to_lowercase().contains(&f_comp.to_lowercase()) {
        return false;
    }
    if !f_proc.is_empty() && !proc.to_lowercase().contains(&f_proc.to_lowercase()) {
        return false;
    }
    if !f_thread.is_empty() && !thread.to_lowercase().contains(&f_thread.to_lowercase()) {
        return false;
    }
    if !matches_user_filter(user, f_users) {
        return false;
    }
    if !f_msg.is_empty() && !msg.to_lowercase().contains(&f_msg.to_lowercase()) {
        return false;
    }
    if !f_source.is_empty() && !source.to_lowercase().contains(&f_source.to_lowercase()) {
        return false;
    }
    if !f_line.is_empty() && !line.to_lowercase().contains(&f_line.to_lowercase()) {
        return false;
    }
    if !f_source_c.is_empty() && !source_c.to_lowercase().contains(&f_source_c.to_lowercase()) {
        return false;
    }
    if !f_line_c.is_empty() && !line_c.to_lowercase().contains(&f_line_c.to_lowercase()) {
        return false;
    }
    if f_hide_triggers && comp.to_uppercase().starts_with("TRIGGER") {
        return false;
    }
    if has_date_to && i < main_end {
        if let Some(ref iso) = iso_dates[i] {
            if iso.as_str() > f_date_to_ref {
                return false;
            }
        }
    }
    if !search_q.is_empty() {
        let haystack = format!("{} {} {}", rec.text, msg, comp).to_lowercase();
        if !search_q
            .split_whitespace()
            .all(|w| haystack.contains(w))
        {
            return false;
        }
    }
    true
}

fn record_to_json(rec: &crate::Record) -> serde_json::Value {
    let comp = rec.variables.get("component").map_or("", |s| s);
    let msg = rec.variables.get("message").map_or(&rec.text, |s| s);
    let is_trigger = comp.to_uppercase().starts_with("TRIGGER");
    let op_class = detect_op_class(msg);
    serde_json::json!({
        "v": rec.variables,
        "c": rec.color,
        "op": op_class,
        "tr": is_trigger,
    })
}

fn load_records(path: &str) -> Result<CachedDataSet, QueryError> {
    let reader = readers::detect_reader(path).map_err(|_| QueryError::NotFound)?;
    let view = view_for_file(path);
    let mut records: Vec<crate::Record> = process(reader, view).filter_map(|r| r.ok()).collect();
    records.reverse();
    let mut with_ts: Vec<(crate::Record, Option<String>)> = Vec::with_capacity(records.len());
    for rec in records {
        let ds = rec
            .variables
            .get("timestamp")
            .or_else(|| rec.variables.get("time"))
            .cloned()
            .unwrap_or_default();
        if ds.is_empty() {
            with_ts.push((rec, None));
        } else {
            with_ts.push((rec, date_str_to_iso(&ds)));
        }
    }
    let mut i = 0;
    for j in 0..with_ts.len() {
        if with_ts[j].1.is_some() {
            if i != j {
                with_ts.swap(i, j);
            }
            i += 1;
        }
    }
    let iso_dates: Vec<Option<String>> = with_ts.iter().map(|(_, iso)| iso.clone()).collect();
    let records: Vec<crate::Record> = with_ts.into_iter().map(|(r, _)| r).collect();
    Ok(CachedDataSet { records, iso_dates })
}

fn cached_records(state: &AppState, path: &str, refresh: bool) -> Result<Arc<CachedDataSet>, QueryError> {
    if refresh {
        state.cache.write().unwrap().remove(path);
    } else if let Some(data) = state.cache.read().unwrap().get(path) {
        return Ok(Arc::clone(data));
    }
    let data = Arc::new(load_records(path)?);
    state
        .cache
        .write()
        .unwrap()
        .insert(path.to_owned(), Arc::clone(&data));
    Ok(data)
}

pub fn view_for_file(path: &str) -> View {
    let fname = std::path::Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let view_file = if fname.to_lowercase().contains("reu") {
        "view_reu.json"
    } else if fname.ends_with(".OUT") || fname.ends_with(".out") {
        "view_core.json"
    } else {
        ""
    };
    if view_file.is_empty() {
        return View { operations: vec![] };
    }
    let view_path = std::path::Path::new(path)
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join(view_file);
    if let Ok(file) = File::open(&view_path) {
        if let Ok(v) = serde_json::from_reader(file) {
            return v;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        let exe_dir = exe.parent().unwrap_or(std::path::Path::new("."));
        let view_path = exe_dir.join(view_file);
        if let Ok(file) = File::open(&view_path) {
            if let Ok(v) = serde_json::from_reader(file) {
                return v;
            }
        }
        let view_path = exe_dir.join("../Resources").join(view_file);
        if let Ok(file) = File::open(&view_path) {
            if let Ok(v) = serde_json::from_reader(file) {
                return v;
            }
        }
    }
    if let Ok(file) = File::open(view_file) {
        if let Ok(v) = serde_json::from_reader(file) {
            return v;
        }
    }
    embedded_view(view_file).unwrap_or_else(|| View { operations: vec![] })
}

fn embedded_view(view_file: &str) -> Option<View> {
    const VIEW_CORE: &str = include_str!("../view_core.json");
    const VIEW_REU: &str = include_str!("../view_reu.json");
    let json = match view_file {
        "view_core.json" => VIEW_CORE,
        "view_reu.json" => VIEW_REU,
        _ => return None,
    };
    serde_json::from_str(json).ok()
}

pub fn parse_user_filters(raw: &str) -> Vec<String> {
    if raw.is_empty() {
        return Vec::new();
    }
    raw.split(',')
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(|u| u.to_lowercase())
        .collect()
}

pub fn matches_user_filter(user: &str, filters: &[String]) -> bool {
    if filters.is_empty() {
        return true;
    }
    let user_lc = user.to_lowercase();
    filters.iter().any(|f| user_lc == *f)
}

fn detect_op_class(msg: &str) -> &'static str {
    let first = msg
        .trim_start()
        .split(|c: char| c.is_whitespace() || c == '(')
        .next()
        .unwrap_or("");
    match first.to_uppercase().as_str() {
        "SELECT" | "WITH" => "select",
        "INSERT" | "MERGE" => "insert",
        "UPDATE" => "update",
        "DELETE" | "DROP" | "TRUNCATE" => "delete",
        "CREATE" | "ALTER" | "BEGIN" => "create",
        "EXEC" | "EXECUTE" | "CALL" | "DECLARE" => "exec",
        "COMMIT" => "commit",
        _ => "",
    }
}

pub fn date_str_to_iso(s: &str) -> Option<String> {
    let parts: Vec<&str> = s.splitn(2, ' ').collect();
    let date_part = parts[0];
    let time_part = parts
        .get(1)
        .map(|t| t.split(',').next().unwrap_or(t))
        .unwrap_or("00:00:00");
    let dp: Vec<&str> = date_part.split('/').collect();
    if dp.len() != 3 {
        return None;
    }
    let v0 = dp[0].parse::<u32>().ok()?;
    let v1 = dp[1].parse::<u32>().ok()?;
    let v2 = dp[2].parse::<u32>().ok()?;
    let (d, m, y) = if s.contains(',') {
        (v0, v1, v2)
    } else {
        (v2, v1, v0)
    };
    let y = if y < 100 { y + 2000 } else { y };
    Some(format!("{:04}-{:02}-{:02}T{}", y, m, d, time_part))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_str_to_iso_core_format() {
        assert_eq!(
            date_str_to_iso("01/07/26 10:00:00,000"),
            Some("2026-07-01T10:00:00".to_owned())
        );
    }

    #[test]
    fn date_str_to_iso_reu_format() {
        assert_eq!(
            date_str_to_iso("25/07/26 10:00:00"),
            Some("2025-07-26T10:00:00".to_owned())
        );
    }

    #[test]
    fn parse_user_filters_splits_comma_separated_values() {
        assert_eq!(
            parse_user_filters("Alice,Bob"),
            vec!["alice".to_owned(), "bob".to_owned()]
        );
        assert!(parse_user_filters("").is_empty());
    }

    #[test]
    fn matches_user_filter_uses_or_semantics() {
        let filters = vec!["alice".to_owned(), "bob".to_owned()];
        assert!(matches_user_filter("Alice", &filters));
        assert!(matches_user_filter("bob", &filters));
        assert!(!matches_user_filter("carol", &filters));
        assert!(matches_user_filter("anyone", &[]));
    }

    #[test]
    fn embedded_view_loads_core_and_reu() {
        assert!(embedded_view("view_core.json").is_some());
        assert!(embedded_view("view_reu.json").is_some());
        assert!(embedded_view("unknown.json").is_none());
    }
}
