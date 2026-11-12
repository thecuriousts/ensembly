use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn read_at(path: &Path, label: &str) -> String {
    match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => panic!("missing {label}: {err}"),
    }
}

fn enum_variants(src: &str, name: &str) -> BTreeSet<String> {
    let head = format!("pub enum {name}");
    let count = src.matches(&head).count();
    assert_eq!(
        count, 1,
        "expected one {head} in src/record.rs, found {count}"
    );
    let at = src.find(&head).expect("enum");
    let after = &src[at + head.len()..];
    let brace = after
        .find('{')
        .unwrap_or_else(|| panic!("{head} has no body"));
    let inside = balanced_body(&after[brace + 1..]);
    let names = variant_names(inside);
    assert!(!names.is_empty(), "{head} parsed no variants");
    names
}

fn balanced_body(src: &str) -> &str {
    let mut depth = 1i32;
    for (i, c) in src.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[..i];
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced enum body");
}

fn variant_names(body: &str) -> BTreeSet<String> {
    let bytes = body.as_bytes();
    let mut i = 0;
    let mut depth = 0i32;
    let mut names = BTreeSet::new();
    while i < bytes.len() {
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        match bytes[i] {
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth -= 1;
                i += 1;
            }
            b'#' if depth == 0 => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            c if depth == 0 && (c == b'_' || c.is_ascii_alphabetic()) => {
                let start = i;
                i += 1;
                while i < bytes.len() && (bytes[i] == b'_' || bytes[i].is_ascii_alphanumeric()) {
                    i += 1;
                }
                let name = std::str::from_utf8(&bytes[start..i]).expect("utf8");
                if name
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_uppercase())
                {
                    names.insert(name.to_string());
                }
            }
            _ => i += 1,
        }
    }
    names
}

fn section_terms(glossary: &str, heading: &str) -> BTreeSet<String> {
    let mut seen = false;
    let mut terms = BTreeSet::new();
    for line in glossary.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") {
            if seen {
                break;
            }
            if trimmed == heading {
                seen = true;
            }
            continue;
        }
        if !seen || !trimmed.starts_with('|') {
            continue;
        }
        let cell = trimmed
            .trim_matches('|')
            .split('|')
            .next()
            .unwrap_or("")
            .trim();
        if let Some(name) = backtick_type(cell) {
            terms.insert(name.to_string());
        }
    }
    assert!(seen, "glossary missing heading {heading}");
    assert!(!terms.is_empty(), "{heading} lists no terms");
    terms
}

fn backtick_type(cell: &str) -> Option<&str> {
    let name = cell.strip_prefix('`')?.strip_suffix('`')?;
    if !name.is_empty()
        && name.chars().next()?.is_ascii_uppercase()
        && name.chars().all(|c| c.is_ascii_alphanumeric())
    {
        Some(name)
    } else {
        None
    }
}

fn assert_same(kind: &str, code: &BTreeSet<String>, doc: &BTreeSet<String>) {
    let missing: Vec<&String> = code.difference(doc).collect();
    let extra: Vec<&String> = doc.difference(code).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "{kind} drift\ncode: {code:?}\nglossary: {doc:?}\nmissing from glossary: {missing:?}\nlisted but not in code: {extra:?}"
    );
}

#[test]
fn glossary_matches_record_enums() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let record = read_at(&manifest.join("src/record.rs"), "src/record.rs");
    let glossary = read_at(&manifest.join("../../docs/GLOSSARY.md"), "docs/GLOSSARY.md");
    assert_same(
        "record status",
        &enum_variants(&record, "Status"),
        &section_terms(&glossary, "## Record status"),
    );
    assert_same(
        "event kind",
        &enum_variants(&record, "EventKind"),
        &section_terms(&glossary, "## Event kind"),
    );
}
