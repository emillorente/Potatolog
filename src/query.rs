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
                use std::time::SystemTime;
                let ts = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let _ = writeln!(file, "[{ts}] {msg}");
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
    // Security: only allow files from temp dir or the default file
    let allowed = std::env::temp_dir().join("logviewer");
    let path_ref = std::path::Path::new(&path);
    if !path_ref.starts_with(&allowed)
        && state.default_file.as_ref().map_or(true, |d| path_ref != std::path::Path::new(d))
    {
        return Ok(QueryResponse { r: vec![], t: 0 });
    }
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
    let f_show_triggers = params.get("showTriggers").map(|v| v == "true").unwrap_or(false);

    let has_date_from = !f_date_from.is_empty();
    let has_date_to = !f_date_to.is_empty();
    let main_end = all_records.len() - empty_ts;

    let has_text_filters = !f_level.is_empty()
        || !f_comp.is_empty()
        || !f_proc.is_empty()
        || !f_thread.is_empty()
        || !f_users.is_empty()
        || !f_msg.is_empty()
        || !f_source.is_empty()
        || !f_line.is_empty()
        || !f_source_c.is_empty()
        || !f_line_c.is_empty();

    if !has_text_filters && !has_date_from && !has_date_to {
        if f_show_triggers {
            // No filters at all — fast path
            let total = all_records.len();
            let out_records: Vec<serde_json::Value> = all_records
                .iter()
                .skip(skip)
                .take(limit)
                .map(record_to_json)
                .collect();
            return Ok(QueryResponse { r: out_records, t: total });
        }
        // Only trigger filter — lightweight pass
        let matching: Vec<usize> = all_records
            .iter()
            .enumerate()
            .filter(|(_, rec)| {
                let comp = rec.get("component").unwrap_or("");
                !(comp.len() >= 7 && comp.as_bytes()[..7].eq_ignore_ascii_case(b"TRIGGER"))
            })
            .map(|(i, _)| i)
            .collect();
        let total = matching.len();
        let out_records: Vec<serde_json::Value> = matching
            .iter()
            .skip(skip)
            .take(limit)
            .map(|&i| record_to_json(&all_records[i]))
            .collect();
        return Ok(QueryResponse { r: out_records, t: total });
    }

    let f_level_lc = f_level.to_ascii_lowercase();
    let f_comp_lc = f_comp.to_ascii_lowercase();
    let f_proc_lc = f_proc.to_ascii_lowercase();
    let f_thread_lc = f_thread.to_ascii_lowercase();
    let f_msg_lc = f_msg.to_ascii_lowercase();
    let f_source_lc = f_source.to_ascii_lowercase();
    let f_line_lc = f_line.to_ascii_lowercase();
    let f_source_c_lc = f_source_c.to_ascii_lowercase();
    let f_line_c_lc = f_line_c.to_ascii_lowercase();


    let date_break = if has_date_from {
        // Binary search: records are newest-first, dates are ISO strings
        // Find the first record where date < f_date_from
        let mut lo = 0usize;
        let mut hi = main_end;
        while lo < hi {
            let mid = (lo + hi) / 2;
            match &iso_dates[mid] {
                Some(iso) if iso.as_str() >= f_date_from => lo = mid + 1,
                _ => hi = mid,
            }
        }
        lo
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
                iso_dates,
                &f_level_lc,
                &f_comp_lc,
                &f_proc_lc,
                &f_thread_lc,
                &f_users,
                &f_msg_lc,
                &f_source_lc,
                &f_line_lc,
                &f_source_c_lc,
                &f_line_c_lc,
                f_show_triggers,
                has_date_to,
                f_date_to_ref,
            )
        })
        .map(|(i, _)| i)
        .chain((main_end..all_records.len()).into_par_iter().filter(|&i| {
            record_matches(
                &all_records[i],
                i,
                iso_dates,
                &f_level_lc,
                &f_comp_lc,
                &f_proc_lc,
                &f_thread_lc,
                &f_users,
                &f_msg_lc,
                &f_source_lc,
                &f_line_lc,
                &f_source_c_lc,
                &f_line_c_lc,
                f_show_triggers,
                false,
                f_date_to_ref,
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
    iso_dates: &[Option<String>],
    f_level_lc: &str,
    f_comp_lc: &str,
    f_proc_lc: &str,
    f_thread_lc: &str,
    f_users: &[String],
    f_msg_lc: &str,
    f_source_lc: &str,
    f_line_lc: &str,
    f_source_c_lc: &str,
    f_line_c_lc: &str,
    f_show_triggers: bool,
    has_date_to: bool,
    f_date_to_ref: &str,
) -> bool {
    // Single pass to extract fields (avoids 11 linear searches)
    let mut level = "";
    let mut comp = "";
    let mut proc = "";
    let mut thread = "";
    let mut user = "";
    let mut msg = rec.text.as_str();
    let mut source = "";
    let mut line = "";
    let mut source_c = "";
    let mut line_c = "";
    for (k, v) in &rec.variables {
        let s = v.as_str();
        match k.as_str() {
            "level" => level = s,
            "component" => comp = s,
            "proc" => proc = s,
            "thread" => thread = s,
            "user" => user = s,
            "message" => msg = s,
            "source" => source = s,
            "line" => line = s,
            "sourceC" => source_c = s,
            "lineC" => line_c = s,
            _ => {}
        }
    }

    if !f_level_lc.is_empty() && !level.to_ascii_lowercase().contains(f_level_lc) {
        return false;
    }
    if !f_comp_lc.is_empty() && !comp.to_ascii_lowercase().contains(f_comp_lc) {
        return false;
    }
    if !f_proc_lc.is_empty() && !proc.to_ascii_lowercase().contains(f_proc_lc) {
        return false;
    }
    if !f_thread_lc.is_empty() && !thread.to_ascii_lowercase().contains(f_thread_lc) {
        return false;
    }
    if !matches_user_filter(user, f_users) {
        return false;
    }
    if !f_msg_lc.is_empty() && !msg.to_ascii_lowercase().contains(f_msg_lc) {
        return false;
    }
    if !f_source_lc.is_empty() && !source.to_ascii_lowercase().contains(f_source_lc) {
        return false;
    }
    if !f_line_lc.is_empty() && !line.to_ascii_lowercase().contains(f_line_lc) {
        return false;
    }
    if !f_source_c_lc.is_empty() && !source_c.to_ascii_lowercase().contains(f_source_c_lc) {
        return false;
    }
    if !f_line_c_lc.is_empty() && !line_c.to_ascii_lowercase().contains(f_line_c_lc) {
        return false;
    }
    if !f_show_triggers && comp.len() >= 7 && comp.as_bytes()[..7].eq_ignore_ascii_case(b"TRIGGER") {
        return false;
    }
    if has_date_to {
        if let Some(ref iso) = iso_dates[i] {
            if iso.as_str() > f_date_to_ref {
                return false;
            }
        }
    }
    true
}

fn record_to_json(rec: &crate::Record) -> serde_json::Value {
    let mut comp = "";
    let mut msg = rec.text.as_str();
    for (k, v) in &rec.variables {
        let s = v.as_str();
        match k.as_str() {
            "component" => comp = s,
            "message" => msg = s,
            _ => {}
        }
    }
    let is_trigger = comp.len() >= 7 && comp.as_bytes()[..7].eq_ignore_ascii_case(b"TRIGGER");
    let op_class = detect_op_class(msg);
    let mut vars = serde_json::Map::with_capacity(rec.variables.len());
    for (k, v) in &rec.variables {
        vars.insert(k.clone(), serde_json::Value::String(v.clone()));
    }
    serde_json::json!({
        "v": serde_json::Value::Object(vars),
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
            .get("timestamp")
            .or_else(|| rec.get("time"))
            .unwrap_or("")
            .to_string();
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
    // Quick check with read lock
    if !refresh {
        if let Some(data) = state.cache.read().unwrap().get(path) {
            return Ok(Arc::clone(data));
        }
    }
    // Load outside any lock
    let data = Arc::new(load_records(path)?);
    // Insert with write lock (brief)
    let mut cache = state.cache.write().unwrap();
    if refresh {
        cache.remove(path);
    }
    cache.insert(path.to_owned(), Arc::clone(&data));
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
    filters.iter().any(|f| f.len() == user.len() && f.as_bytes().eq_ignore_ascii_case(user.as_bytes()))
}

fn detect_op_class(msg: &str) -> &'static str {
    let first = msg
        .trim_start()
        .split(|c: char| c.is_whitespace() || c == '(')
        .next()
        .unwrap_or("");
    if first.len() < 2 { return ""; }
    let b = first.as_bytes();
    match b[0] | 32 {
        b's' if (b.len() == 6 && b[1..].eq_ignore_ascii_case(b"elect"))
            || (b.len() == 4 && b[1..].eq_ignore_ascii_case(b"with")) => "select",
        b'i' if b.len() == 6 && b[1..].eq_ignore_ascii_case(b"nsert") => "insert",
        b'u' if b.len() == 6 && b[1..].eq_ignore_ascii_case(b"pdate") => "update",
        b'd' if (b.len() == 6 && b[1..].eq_ignore_ascii_case(b"elete"))
            || (b.len() == 4 && b[1..].eq_ignore_ascii_case(b"rop"))
            || (b.len() == 8 && b[1..].eq_ignore_ascii_case(b"runcate")) => "delete",
        b'c' if (b.len() == 6 && b[1..].eq_ignore_ascii_case(b"reate"))
            || (b.len() == 5 && b[1..].eq_ignore_ascii_case(b"lter")) => "create",
        b'm' if b.len() == 5 && b[1..].eq_ignore_ascii_case(b"erge") => "merge",
        b'b' if b.len() == 5 && b[1..].eq_ignore_ascii_case(b"egin") => "begin",
        b'e' if (b.len() == 4 && b[1..].eq_ignore_ascii_case(b"xec"))
            || (b.len() == 7 && b[1..].eq_ignore_ascii_case(b"xecute"))
            || (b.len() == 4 && b[1..].eq_ignore_ascii_case(b"all"))
            || (b.len() == 7 && b[1..].eq_ignore_ascii_case(b"eclare")) => "exec",
        b'c' if b.len() == 6 && b[1..].eq_ignore_ascii_case(b"ommit") => "commit",
        _ => "",
    }
}

pub fn date_str_to_iso(s: &str) -> Option<String> {
    let space = s.find(' ').unwrap_or(s.len());
    let date_part = &s[..space];
    let time_part = if space < s.len() {
        let after_space = &s[space + 1..];
        after_space.split(',').next().unwrap_or(after_space)
    } else {
        "00:00:00"
    };
    let mut segs = date_part.split('/');
    let v0 = segs.next()?.parse::<u32>().ok()?;
    let v1 = segs.next()?.parse::<u32>().ok()?;
    let v2 = segs.next()?.parse::<u32>().ok()?;
    if segs.next().is_some() { return None; }
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
