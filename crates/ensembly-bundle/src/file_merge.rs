use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde_json::Value;

fn ts_suffix() -> String {
    Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
}

fn files_equal(a: &Path, b: &Path) -> Result<bool, String> {
    let sa = fs::read(a).map_err(|e| e.to_string())?;
    let sb = fs::read(b).map_err(|e| e.to_string())?;
    Ok(sa == sb)
}

fn sidecar_path(dest: &Path) -> PathBuf {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    let stem = dest.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = dest
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    parent.join(format!("{stem}.incoming-{ts}{ext}", ts = ts_suffix()))
}

fn jsonl_record_id(line: &str) -> Result<String, String> {
    let v: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    v.get("id")
        .or_else(|| v.get("key"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "jsonl record missing id or key field".into())
}

pub fn merge_jsonl_records(src: &Path, dest: &Path) -> Result<Vec<PathBuf>, String> {
    let existing_raw = fs::read_to_string(dest).map_err(|e| e.to_string())?;
    let incoming_raw = fs::read_to_string(src).map_err(|e| e.to_string())?;
    let mut by_id: BTreeMap<String, String> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for line in existing_raw.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let id = jsonl_record_id(t)?;
        if !by_id.contains_key(&id) {
            order.push(id.clone());
        }
        by_id.insert(id, t.to_string());
    }
    let mut conflict_lines: Vec<String> = Vec::new();
    for line in incoming_raw.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let id = jsonl_record_id(t)?;
        match by_id.get(&id) {
            None => {
                order.push(id.clone());
                by_id.insert(id, t.to_string());
            }
            Some(prev) if prev == t => {}
            Some(_) => conflict_lines.push(t.to_string()),
        }
    }
    let merged = order
        .iter()
        .filter_map(|id| by_id.get(id))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let body = if merged.is_empty() {
        String::new()
    } else {
        format!("{merged}\n")
    };
    fs::write(dest, body).map_err(|e| e.to_string())?;
    let mut sidecars = Vec::new();
    if !conflict_lines.is_empty() {
        let side = sidecar_path(dest);
        let body = format!("{}\n", conflict_lines.join("\n"));
        fs::write(&side, body).map_err(|e| e.to_string())?;
        eprintln!(
            "warn: jsonl id conflicts; kept existing {}; incoming → {}",
            dest.display(),
            side.display()
        );
        sidecars.push(side);
    }
    Ok(sidecars)
}

pub fn merge_json_objects(src: &Path, dest: &Path) -> Result<Vec<PathBuf>, String> {
    let existing: Value = serde_json::from_slice(&fs::read(dest).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let incoming: Value = serde_json::from_slice(&fs::read(src).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let (Some(mut out), Some(inp)) = (existing.as_object().cloned(), incoming.as_object()) else {
        return merge_other_file(src, dest);
    };
    let mut conflict = false;
    for (k, v) in inp {
        match out.get(k) {
            None => {
                out.insert(k.clone(), v.clone());
            }
            Some(ev) if ev == v => {}
            Some(_) => conflict = true,
        }
    }
    fs::write(
        dest,
        serde_json::to_vec_pretty(&Value::Object(out)).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut sidecars = Vec::new();
    if conflict {
        let side = sidecar_path(dest);
        fs::copy(src, &side).map_err(|e| e.to_string())?;
        eprintln!(
            "warn: json key conflict; kept merged base {}; full incoming → {}",
            dest.display(),
            side.display()
        );
        sidecars.push(side);
    }
    Ok(sidecars)
}

pub fn merge_other_file(src: &Path, dest: &Path) -> Result<Vec<PathBuf>, String> {
    if files_equal(src, dest)? {
        return Ok(vec![]);
    }
    let side = sidecar_path(dest);
    fs::copy(src, &side).map_err(|e| e.to_string())?;
    eprintln!(
        "warn: kept existing {}; incoming → {}",
        dest.display(),
        side.display()
    );
    Ok(vec![side])
}

pub fn restore_file_with_merge(
    src: &Path,
    dest: &Path,
    merge: bool,
) -> Result<Vec<PathBuf>, String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if !dest.exists() {
        fs::copy(src, dest).map_err(|e| e.to_string())?;
        return Ok(vec![]);
    }
    if !merge {
        return Err(format!(
            "refusing overwrite {}; pass --merge",
            dest.display()
        ));
    }
    if dest.extension().and_then(|s| s.to_str()) == Some("jsonl")
        || dest.to_string_lossy().ends_with(".jsonl")
    {
        return merge_jsonl_records(src, dest);
    }
    if dest.extension().and_then(|s| s.to_str()) == Some("json") {
        return merge_json_objects(src, dest);
    }
    merge_other_file(src, dest)
}
