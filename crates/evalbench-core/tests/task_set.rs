//! Frozen synthetic task set and its hash (method documented in src/taskset.rs).
mod common;
use common::*;
use evalbench_core::taskset::{manifest, set_hash, FixtureFile, TaskSet, TaskSetError};

const IDS: [&str; 12] = [
    "t01-full",
    "t02-gappy",
    "t03-stripped",
    "t04-empty-evidence",
    "t05-case",
    "t06-substring-trap",
    "t07-empty-proof",
    "t08-split-item",
    "t09-data-full",
    "t10-data-partial",
    "t11-multi-hit",
    "t12-no-proof-key",
];

#[test]
fn frozen_hash_matches_committed_file() {
    let raw = std::fs::read(fixtures().join("tasks.sha256")).unwrap();
    assert_eq!(raw.len(), 65, "tasks.sha256 is 64 hex + \\n");
    let files = fixture_files(&tasks_dir());
    assert_eq!(set_hash(&files), committed_hash());
    assert_eq!(frozen_set().hash, committed_hash());
}

#[test]
fn manifest_is_sha256sum_format_and_sorted() {
    let m = manifest(&fixture_files(&tasks_dir()));
    assert!(m.ends_with('\n'));
    let lines: Vec<&str> = m.lines().collect();
    assert_eq!(lines.len(), 36);
    let paths: Vec<&str> = lines.iter().map(|l| &l[66..]).collect();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted);
    for l in &lines {
        let (hex, rest) = l.split_at(64);
        assert!(
            hex.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "{l}"
        );
        assert!(rest.starts_with("  t") && !rest.contains('\\'), "{l}");
    }
    assert_eq!(sha256_hex(m.as_bytes()), committed_hash());
}

#[test]
fn hash_changes_on_any_byte_or_rename() {
    let files = fixture_files(&tasks_dir());
    let base = set_hash(&files);
    let mut byte = files.clone();
    byte[0].bytes.push(b' ');
    assert_ne!(set_hash(&byte), base);
    let mut renamed = files.clone();
    renamed[0].rel_path = renamed[0].rel_path.replacen("t", "u", 1);
    assert_ne!(set_hash(&renamed), base);
}

#[test]
fn hash_is_independent_of_input_order() {
    let files = fixture_files(&tasks_dir());
    let mut rev = files.clone();
    rev.reverse();
    assert_eq!(set_hash(&files), set_hash(&rev));
}

#[test]
fn task_set_rejects_stray_missing_or_bad_id() {
    let files = fixture_files(&tasks_dir());

    let mut stray = files.clone();
    stray.push(FixtureFile {
        rel_path: "t01-full/.DS_Store".into(),
        bytes: vec![0],
    });
    assert!(
        matches!(TaskSet::from_files(stray), Err(TaskSetError::StrayFile(p)) if p.contains(".DS_Store"))
    );

    let missing: Vec<FixtureFile> = files
        .iter()
        .filter(|f| f.rel_path != "t03-stripped/expected.json")
        .cloned()
        .collect();
    assert!(matches!(
        TaskSet::from_files(missing),
        Err(TaskSetError::MissingFile { task, file }) if task == "t03-stripped" && file == "expected.json"
    ));

    let bad_id: Vec<FixtureFile> = files
        .iter()
        .map(|f| FixtureFile {
            rel_path: f.rel_path.replace("t01-full", "T01"),
            bytes: f.bytes.clone(),
        })
        .collect();
    assert!(matches!(TaskSet::from_files(bad_id), Err(TaskSetError::BadTaskId(id)) if id == "T01"));
}

#[test]
fn task_set_rejects_expected_mismatch() {
    let mut files = fixture_files(&tasks_dir());
    let f = files
        .iter_mut()
        .find(|f| f.rel_path == "t02-gappy/expected.json")
        .unwrap();
    f.bytes = b"{\"gaps\":[\"RAG\"]}\n".to_vec();
    assert!(matches!(
        TaskSet::from_files(files),
        Err(TaskSetError::ExpectedMismatch { task, .. }) if task == "t02-gappy"
    ));
}

#[test]
fn frozen_set_is_twelve_tasks_sorted_by_id() {
    let set = frozen_set();
    assert_eq!(set.ids(), names(&IDS));
    let t02 = &set.tasks[1];
    assert_eq!(
        t02.expected_gaps,
        names(&["LLM evals", "RAG", "Observability"])
    );
    assert_eq!(
        t02.posting_raw,
        read(&tasks_dir().join("t02-gappy/posting.json"))
    );
}

/// Guard (passes today): fixtures are synthetic. No personal data, mail or home paths.
#[test]
fn fixtures_are_synthetic() {
    for f in fixture_files(&fixtures()) {
        let text = String::from_utf8_lossy(&f.bytes);
        for bad in ["/Users/", "/home/"] {
            assert!(!text.contains(bad), "{} contains {bad:?}", f.rel_path);
        }
        // Email-shaped `@` (a dotted domain follows). `recall@k` is fine.
        for (i, _) in text.match_indices('@') {
            let domain: String = text[i + 1..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-')
                .collect();
            assert!(!domain.contains('.'), "{}: email-like text", f.rel_path);
        }
        for (i, _) in text.match_indices("https://") {
            assert!(
                text[i..].starts_with("https://example.org/"),
                "{}: non-synthetic URL",
                f.rel_path
            );
        }
    }
}
