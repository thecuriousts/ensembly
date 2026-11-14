use std::path::Path;

/// RFC 5545-ish structural checks for portable view calendars.
pub fn verify_ics_content(raw: &str) -> Result<(), String> {
    let unfolded = unfold_ics(raw);
    if !unfolded.contains("BEGIN:VCALENDAR") || !unfolded.contains("END:VCALENDAR") {
        return Err("missing VCALENDAR envelope".into());
    }
    if !unfolded.contains("VERSION:2.0") {
        return Err("missing VERSION:2.0".into());
    }
    let mut depth = 0i32;
    let mut in_vevent = false;
    for line in unfolded.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if line == "BEGIN:VEVENT" {
            if in_vevent {
                return Err("nested VEVENT".into());
            }
            in_vevent = true;
            depth += 1;
        } else if line == "END:VEVENT" {
            if !in_vevent {
                return Err("END:VEVENT without BEGIN".into());
            }
            in_vevent = false;
            depth -= 1;
        } else if line.starts_with("BEGIN:") || line.starts_with("END:") {
            continue;
        } else if in_vevent {
            let Some((name, _)) = line.split_once(':') else {
                return Err(format!("invalid property line: {line}"));
            };
            if name.is_empty() {
                return Err("empty property name".into());
            }
        }
    }
    if in_vevent || depth != 0 {
        return Err("unclosed VEVENT".into());
    }
    Ok(())
}

pub fn verify_ics_file(path: &Path) -> Result<(), String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    verify_ics_content(&raw)
}

fn unfold_ics(raw: &str) -> String {
    let mut out = String::new();
    for line in raw.replace("\r\n", "\n").lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(stripped) = line.strip_prefix([' ', '\t']) {
                out.push_str(stripped);
            }
        } else {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
        }
    }
    out
}
