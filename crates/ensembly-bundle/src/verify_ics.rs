use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// Upper bound for ICS view verification (hostile / accidental huge inputs).
pub const VERIFY_ICS_MAX_BYTES: u64 = 8 * 1024 * 1024;
/// RFC folding should not produce logical lines longer than this in views.
pub const VERIFY_ICS_MAX_LOGICAL_LINE_BYTES: usize = 256 * 1024;
const MAX_ICS_COMPONENT_DEPTH: i32 = 64;

/// RFC 5545-ish structural checks for portable view calendars.
pub fn verify_ics_content(raw: &str) -> Result<(), String> {
    if raw.len() as u64 > VERIFY_ICS_MAX_BYTES {
        return Err(format!(
            "ICS content exceeds {} byte limit",
            VERIFY_ICS_MAX_BYTES
        ));
    }
    verify_ics_reader(raw.as_bytes())
}

pub fn verify_ics_file(path: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > VERIFY_ICS_MAX_BYTES {
        return Err(format!(
            "ICS file too large ({} bytes > {} byte limit)",
            meta.len(),
            VERIFY_ICS_MAX_BYTES
        ));
    }
    let file = File::open(path).map_err(|e| e.to_string())?;
    verify_ics_reader(file)
}

fn verify_ics_reader(reader: impl Read) -> Result<(), String> {
    let mut buf = BufReader::new(reader);
    let mut logical = String::new();
    let mut saw_vcalendar = false;
    let mut saw_end_vcalendar = false;
    let mut saw_version = false;
    let mut in_vevent = false;
    let mut component_depth = 0i32;

    loop {
        let mut physical = Vec::new();
        let bytes = buf
            .read_until(b'\n', &mut physical)
            .map_err(|e| e.to_string())?;
        if bytes == 0 {
            break;
        }
        if physical.len() > VERIFY_ICS_MAX_LOGICAL_LINE_BYTES {
            return Err(format!(
                "ICS physical line exceeds {} byte limit",
                VERIFY_ICS_MAX_LOGICAL_LINE_BYTES
            ));
        }
        let line = String::from_utf8_lossy(&physical);
        let line = line.trim_end_matches(&['\r', '\n'][..]);
        if line.starts_with(' ') || line.starts_with('\t') {
            if logical.is_empty() {
                return Err("fold continuation without preceding line".into());
            }
            let Some(stripped) = line.strip_prefix([' ', '\t']) else {
                return Err("invalid folded line".into());
            };
            if logical.len() + stripped.len() > VERIFY_ICS_MAX_LOGICAL_LINE_BYTES {
                return Err(format!(
                    "ICS unfolded line exceeds {} byte limit",
                    VERIFY_ICS_MAX_LOGICAL_LINE_BYTES
                ));
            }
            logical.push_str(stripped);
            continue;
        }
        if !logical.is_empty() {
            verify_ics_logical_line(
                &logical,
                &mut saw_vcalendar,
                &mut saw_end_vcalendar,
                &mut saw_version,
                &mut in_vevent,
                &mut component_depth,
            )?;
            logical.clear();
        }
        logical = line.to_string();
    }
    if !logical.is_empty() {
        verify_ics_logical_line(
            &logical,
            &mut saw_vcalendar,
            &mut saw_end_vcalendar,
            &mut saw_version,
            &mut in_vevent,
            &mut component_depth,
        )?;
    }
    if !saw_vcalendar {
        return Err("missing VCALENDAR envelope".into());
    }
    if !saw_end_vcalendar {
        return Err("missing END:VCALENDAR".into());
    }
    if !saw_version {
        return Err("missing VERSION:2.0".into());
    }
    if in_vevent {
        return Err("unclosed VEVENT".into());
    }
    if component_depth != 0 {
        return Err("unclosed ICS component".into());
    }
    Ok(())
}

fn verify_ics_logical_line(
    line: &str,
    saw_vcalendar: &mut bool,
    saw_end_vcalendar: &mut bool,
    saw_version: &mut bool,
    in_vevent: &mut bool,
    component_depth: &mut i32,
) -> Result<(), String> {
    let line = line.trim_end_matches(&['\r', '\n'][..]);
    if line.is_empty() {
        return Ok(());
    }
    if line == "BEGIN:VCALENDAR" {
        if *saw_vcalendar && !*saw_end_vcalendar {
            return Err("nested VCALENDAR".into());
        }
        *saw_vcalendar = true;
        *component_depth += 1;
        if *component_depth > MAX_ICS_COMPONENT_DEPTH {
            return Err("ICS nesting too deep".into());
        }
        return Ok(());
    }
    if line == "END:VCALENDAR" {
        *saw_end_vcalendar = true;
        *component_depth -= 1;
        if *component_depth < 0 {
            return Err("END:VCALENDAR without BEGIN".into());
        }
        return Ok(());
    }
    if line == "BEGIN:VEVENT" {
        if *in_vevent {
            return Err("nested VEVENT".into());
        }
        *in_vevent = true;
        *component_depth += 1;
        if *component_depth > MAX_ICS_COMPONENT_DEPTH {
            return Err("ICS nesting too deep".into());
        }
        return Ok(());
    }
    if line == "END:VEVENT" {
        if !*in_vevent {
            return Err("END:VEVENT without BEGIN".into());
        }
        *in_vevent = false;
        *component_depth -= 1;
        if *component_depth < 0 {
            return Err("unexpected END:VEVENT".into());
        }
        return Ok(());
    }
    if line.starts_with("BEGIN:") || line.starts_with("END:") {
        if line.starts_with("BEGIN:") {
            *component_depth += 1;
            if *component_depth > MAX_ICS_COMPONENT_DEPTH {
                return Err("ICS nesting too deep".into());
            }
        } else {
            *component_depth -= 1;
            if *component_depth < 0 {
                return Err(format!("unexpected {line}"));
            }
        }
        return Ok(());
    }
    if line.starts_with("VERSION:2.0") {
        *saw_version = true;
    }
    if *in_vevent {
        let Some((name, _)) = line.split_once(':') else {
            return Err(format!("invalid property line: {line}"));
        };
        if name.is_empty() {
            return Err("empty property name".into());
        }
    }
    Ok(())
}
