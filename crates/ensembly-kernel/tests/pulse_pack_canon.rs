//! E2: canonical pulse pack, shared golden vectors (Rust lib, Rust CLI and the stdlib
//! Python checker), and the read-only store. Synthetic fixtures only.
use ensembly_kernel::pulse_core as pc;
use ensembly_kernel::pulse_pack::canonical as canon;
use ensembly_kernel::pulse_supersede as ps;
use ensembly_kernel::KernelWriter;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard};

static WRITER_ENV: Mutex<()> = Mutex::new(());

fn setup_writer() -> MutexGuard<'static, ()> {
    let guard = WRITER_ENV.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var(ensembly_schema::WRITER_ENV, "laptop-1");
    guard
}

fn golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/pulse-pack")
}

fn vectors() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(golden().join("vectors"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    v.sort();
    v
}

fn pack_vectors() -> Vec<PathBuf> {
    vectors()
        .into_iter()
        .filter(|d| d.join("input.json").is_file())
        .collect()
}

fn db_vectors() -> Vec<PathBuf> {
    vectors()
        .into_iter()
        .filter(|d| d.join("args.json").is_file())
        .collect()
}

/// Stores with the golden `pulse check` evidence and supersession findings.
fn db_check_vectors() -> Vec<PathBuf> {
    vectors()
        .into_iter()
        .filter(|d| d.join("db-check.json").is_file())
        .collect()
}

fn checker() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/pulse/pulse_pack_check.py")
}

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

fn python(args: &[&std::ffi::OsStr]) -> Output {
    Command::new("python3")
        .args(["-I", "-X", "utf8"])
        .arg(checker())
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("python3 not found: differential test cannot run ({e})"))
}

fn python_eval(code: &str, args: &[&std::ffi::OsStr]) -> Output {
    let prelude = "import importlib.util as u, sys\n\
        s = u.spec_from_file_location('p', sys.argv[1]); m = u.module_from_spec(s)\n\
        s.loader.exec_module(m)\n";
    Command::new("python3")
        .args(["-I", "-X", "utf8", "-c"])
        .arg(format!("{prelude}{code}"))
        .arg(checker())
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("python3 not found: differential test cannot run ({e})"))
}

fn ensembly(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .args(args)
        .output()
        .expect("run ensembly")
}

fn os(s: &str) -> &std::ffi::OsStr {
    std::ffi::OsStr::new(s)
}

/// Builds a vector's store in a temp dir; returns (dir guard, db path, exported_at, host).
fn build_db(dir: &Path) -> (tempfile::TempDir, PathBuf, String, String) {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("pulse.sqlite");
    let c = Connection::open(&db).unwrap();
    c.execute_batch(&fs::read_to_string(dir.join("fixture.sql")).unwrap())
        .unwrap();
    drop(c);
    let args: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("args.json")).unwrap()).unwrap();
    (
        tmp,
        db,
        args["exported_at"].as_str().unwrap().to_string(),
        args["host"].as_str().unwrap().to_string(),
    )
}

/// Bytes of the main DB plus the names and sizes of every file beside it.
fn snapshot(db: &Path) -> (String, Vec<(String, u64)>) {
    let mut side: Vec<(String, u64)> = fs::read_dir(db.parent().unwrap())
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                e.metadata().unwrap().len(),
            )
        })
        .filter(|(n, _)| !n.ends_with(".sqlite"))
        .collect();
    side.sort();
    (sha(&fs::read(db).unwrap()), side)
}

/// D-E2-6: a read leaves the main DB bytes unchanged, no rollback journal, and any
/// WAL sidecar empty. `-shm` may appear on a WAL store.
fn assert_untouched(db: &Path, before: &(String, Vec<(String, u64)>)) {
    let after = snapshot(db);
    assert_eq!(after.0, before.0, "main DB bytes changed");
    for (name, len) in &after.1 {
        assert!(!name.ends_with("-journal"), "rollback journal left: {name}");
        if name.ends_with("-wal") {
            assert_eq!(*len, 0, "WAL sidecar holds frames after a read");
        } else if !name.ends_with("-shm") {
            assert!(before.1.iter().any(|(n, _)| n == name), "new file {name}");
        }
    }
}

// ---------------------------------------------------------------- guards

#[test]
fn golden_manifest_is_frozen() {
    let root = golden();
    let manifest = fs::read_to_string(root.join("MANIFEST.sha256")).unwrap();
    let mut want = Vec::new();
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
                out.push((rel, sha(&fs::read(&p).unwrap())));
            }
        }
    }
    walk(&root, &root.join("vectors"), &mut want);
    want.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let text: String = want.iter().map(|(r, h)| format!("{h}  {r}\n")).collect();
    assert_eq!(manifest, text, "a golden changed: regenerate and review");
    assert!(want.len() >= 60, "only {} golden files", want.len());
}

#[test]
fn goldens_are_synthetic() {
    for d in vectors() {
        for e in fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            let text = String::from_utf8_lossy(&fs::read(&p).unwrap()).into_owned();
            for bad in ["/Users/", "/home/", "@"] {
                assert!(!text.contains(bad), "{} contains {bad:?}", p.display());
            }
            if p.file_name().unwrap() == "canonical.json" {
                let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
                let host = doc["host"].as_str().unwrap();
                assert!(["laptop-1", "not-an-alias"].contains(&host), "{host}");
                for t in doc["traces"].as_array().unwrap() {
                    assert!(t["nat_key"].as_str().unwrap().starts_with("demo/"));
                }
            }
        }
    }
}

#[test]
fn shuffled_vector_matches_basic_golden() {
    let v = golden().join("vectors");
    assert_eq!(
        fs::read(v.join("05-shuffled/canonical.json")).unwrap(),
        fs::read(v.join("02-python-basic/canonical.json")).unwrap()
    );
    assert_ne!(
        fs::read(v.join("05-shuffled/input.json")).unwrap(),
        fs::read(v.join("02-python-basic/input.json")).unwrap()
    );
}

// ---------------------------------------------------------------- rules

#[test]
fn canonical_json_sorts_by_code_point_and_escapes_minimally() {
    let v: serde_json::Value = serde_json::from_str(
        r#"{"b":"\u0007\t\"\\ \u2028/\u007f é","a":[1,null,true],"ｚ":1,"😀":2}"#,
    )
    .unwrap();
    assert_eq!(
        canon::canonical_json(&v).unwrap(),
        "{\"a\":[1,null,true],\"b\":\"\\u0007\\t\\\"\\\\ \u{2028}/\u{7f} é\",\"ｚ\":1,\"😀\":2}"
    );
    let f: serde_json::Value = serde_json::from_str("[1.5]").unwrap();
    assert_eq!(canon::canonical_json(&f).unwrap_err().code, "E_FLOAT");
}

#[test]
fn parse_strict_rejects_bom_nan_dup_keys_lone_surrogate_bad_utf8() {
    for bad in [
        &b"\xef\xbb\xbf{}"[..],
        b"{\"a\":NaN}",
        b"{\"a\":1,\"a\":2}",
        b"{\"a\":\"\\ud800\"}",
        b"{\"a\":\"\xff\"}",
        b"{} {}",
    ] {
        assert_eq!(
            canon::parse_strict(bad).unwrap_err().code,
            "E_JSON",
            "{bad:?}"
        );
    }
    assert!(canon::parse_strict(b" {\"a\":[1,2]}\n").is_ok());
}

const TS_TABLE: [(&str, Option<&str>); 16] = [
    ("2026-11-13T12:00:00Z", Some("2026-11-13T12:00:00Z")),
    ("2026-11-13T12:00:00+00:00", Some("2026-11-13T12:00:00Z")),
    (
        "2026-11-13T12:00:00.500000+02:00",
        Some("2026-11-13T10:00:00.5Z"),
    ),
    ("2026-11-13T12:00:00.000Z", Some("2026-11-13T12:00:00Z")),
    (
        "2026-01-01T00:30:00.123456789+01:00",
        Some("2025-12-31T23:30:00.123456789Z"),
    ),
    ("2026-11-13 08:05", Some("2026-11-13T08:05:00Z")),
    ("2026-11-13", Some("2026-11-13T00:00:00Z")),
    ("2024-02-29", Some("2024-02-29T00:00:00Z")),
    ("2026-02-29", None),
    ("2026-11-13T12:00:60Z", None),
    ("2026-11-13T12:00:00z", None),
    ("2026-11-13t12:00:00Z", None),
    ("2026-11-13T12:00:00", None),
    ("2026-11-13T12:00:00.1234567890Z", None),
    ("9999-12-31T23:30:00-01:00", None),
    ("２026-11-13", None),
];

#[test]
fn canonical_ts_table() {
    for (input, want) in TS_TABLE {
        assert_eq!(canon::canonical_ts(input).as_deref(), want, "{input}");
    }
}

#[test]
fn python_canonical_ts_matches_rust_table() {
    let inputs: Vec<&str> = TS_TABLE.iter().map(|(i, _)| *i).collect();
    let o = python_eval(
        "import json\nfor t in json.loads(sys.argv[2]): print(json.dumps(m.canonical_ts(t)))",
        &[os(&serde_json::to_string(&inputs).unwrap())],
    );
    assert!(o.status.success(), "{o:?}");
    let got: Vec<Option<String>> = String::from_utf8(o.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let want: Vec<Option<String>> = TS_TABLE
        .iter()
        .map(|(_, w)| w.map(str::to_string))
        .collect();
    assert_eq!(got, want);
}

#[test]
fn content_hash_is_prefixed_sha256() {
    assert_eq!(
        canon::content_hash("abc"),
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

// ---------------------------------------------------------------- goldens, Rust

#[test]
fn rust_reproduces_pack_goldens() {
    let mut n = 0;
    for d in pack_vectors() {
        let input = fs::read(d.join("input.json")).unwrap();
        let (doc, verdict) = canon::check_bytes(&input);
        assert_eq!(
            String::from_utf8(canon::verdict_bytes(&verdict)).unwrap(),
            fs::read_to_string(d.join("check.json")).unwrap(),
            "{}",
            d.display()
        );
        let canonical = d.join("canonical.json");
        match doc {
            Some(doc) => {
                let bytes = canon::canonical_bytes(&doc).unwrap();
                assert_eq!(bytes, fs::read(&canonical).unwrap(), "{}", d.display());
                assert_eq!(
                    format!("{}\n", sha(&bytes)),
                    fs::read_to_string(d.join("sha256.txt")).unwrap()
                );
                assert_eq!(verdict.pack_sha256.as_deref(), Some(sha(&bytes).as_str()));
            }
            None => assert!(!canonical.exists(), "{}", d.display()),
        }
        n += 1;
    }
    assert!(n >= 20, "only {n} pack vectors");
}

#[test]
fn rust_canonical_is_idempotent() {
    for d in vectors() {
        let path = d.join("canonical.json");
        let Ok(bytes) = fs::read(&path) else { continue };
        let again = canon::canonicalize(&bytes).unwrap_or_else(|e| panic!("{d:?}: {e:?}"));
        assert_eq!(canon::canonical_bytes(&again).unwrap(), bytes, "{d:?}");
    }
}

#[test]
fn old_rust_packs_map_to_the_canonical_format() {
    for (name, src) in [
        ("06-rust-format", "ensembly-pulse-pack-v1"),
        ("07-rust-legacy-empty", "peram-pulse-pack-v1"),
    ] {
        let d = golden().join("vectors").join(name);
        let doc = canon::canonicalize(&fs::read(d.join("input.json")).unwrap()).unwrap();
        assert_eq!(doc["format"], canon::CANON_FORMAT);
        assert_eq!(doc["ext"]["source_format"], src);
    }
}

#[test]
fn rust_from_db_reproduces_goldens_and_leaves_db_untouched() {
    for d in db_vectors() {
        let (_tmp, db, at, host) = build_db(&d);
        let before = snapshot(&db);
        let (doc, verdict) = canon::canonical_from_db_path(&db, &at, &host).unwrap();
        assert_eq!(
            canon::canonical_bytes(&doc.unwrap()).unwrap(),
            fs::read(d.join("canonical.json")).unwrap(),
            "{}",
            d.display()
        );
        assert_eq!(
            canon::verdict_bytes(&verdict),
            fs::read(d.join("check.json")).unwrap()
        );
        assert_untouched(&db, &before);
    }
}

#[test]
fn cli_canon_and_pack_check_reproduce_goldens() {
    for d in pack_vectors() {
        let input = d.join("input.json");
        let check = ensembly(&[
            os("pulse"),
            os("pack-check"),
            input.as_os_str(),
            os("--json"),
        ]);
        let want = fs::read(d.join("check.json")).unwrap();
        assert_eq!(check.stdout, want, "{}", d.display());
        let v: serde_json::Value = serde_json::from_slice(&want).unwrap();
        assert_eq!(
            check.status.code(),
            Some(v["exit"].as_i64().unwrap() as i32)
        );
        let c = ensembly(&[os("pulse"), os("canon"), input.as_os_str()]);
        match fs::read(d.join("canonical.json")) {
            Ok(bytes) => {
                assert_eq!(c.status.code(), Some(0), "{c:?}");
                assert_eq!(c.stdout, bytes, "{}", d.display());
            }
            Err(_) => {
                assert_eq!(c.status.code(), Some(2), "{c:?}");
                assert!(c.stdout.is_empty());
                assert_eq!(c.stderr, want, "{}", d.display());
            }
        }
    }
    for d in db_vectors() {
        let (_tmp, db, at, host) = build_db(&d);
        let before = snapshot(&db);
        let c = ensembly(&[
            os("pulse"),
            os("--pulse-db"),
            db.as_os_str(),
            os("canon"),
            os("--from-db"),
            os("--exported-at"),
            os(&at),
            os("--host"),
            os(&host),
        ]);
        assert_eq!(c.status.code(), Some(0), "{c:?}");
        assert_eq!(c.stdout, fs::read(d.join("canonical.json")).unwrap());
        assert_untouched(&db, &before);
    }
}

// ---------------------------------------------------------------- goldens, Python

#[test]
fn python_reproduces_pack_goldens() {
    for d in pack_vectors() {
        let input = d.join("input.json");
        let check = python(&[os("check"), input.as_os_str()]);
        let want = fs::read(d.join("check.json")).unwrap();
        assert_eq!(check.stdout, want, "{}", d.display());
        let v: serde_json::Value = serde_json::from_slice(&want).unwrap();
        assert_eq!(
            check.status.code(),
            Some(v["exit"].as_i64().unwrap() as i32)
        );
        let c = python(&[os("canon"), input.as_os_str()]);
        match fs::read(d.join("canonical.json")) {
            Ok(bytes) => {
                assert_eq!(c.status.code(), Some(0), "{c:?}");
                assert_eq!(c.stdout, bytes, "{}", d.display());
                let again = python(&[os("canon"), d.join("canonical.json").as_os_str()]);
                assert_eq!(again.stdout, bytes, "python idempotence {}", d.display());
            }
            Err(_) => {
                assert_eq!(c.status.code(), Some(2), "{c:?}");
                assert!(c.stdout.is_empty());
                assert_eq!(c.stderr, want, "{}", d.display());
            }
        }
    }
}

#[test]
fn python_from_db_reproduces_goldens_and_leaves_db_untouched() {
    for d in db_vectors() {
        let (_tmp, db, at, host) = build_db(&d);
        let before = snapshot(&db);
        let c = python(&[
            os("canon"),
            os("--db"),
            db.as_os_str(),
            os("--exported-at"),
            os(&at),
            os("--host"),
            os(&host),
        ]);
        assert_eq!(c.status.code(), Some(0), "{c:?}");
        assert_eq!(c.stdout, fs::read(d.join("canonical.json")).unwrap());
        assert_untouched(&db, &before);
    }
}

// ---------------------------------------------------------------- read-only store

const ODD_PATHS: [&str; 3] = [
    "/tmp/pulse dir/a?b#c%d é.sqlite",
    "relative/pulse.sqlite",
    "/tmp/plain-name_1.~x",
];

#[test]
fn ro_uri_is_exact_and_matches_python() {
    assert_eq!(
        pc::ro_uri(Path::new(ODD_PATHS[0])),
        "file:/tmp/pulse%20dir/a%3Fb%23c%25d%20%C3%A9.sqlite?mode=ro"
    );
    assert_eq!(
        pc::ro_uri(Path::new(ODD_PATHS[1])),
        "file:relative/pulse.sqlite?mode=ro"
    );
    for p in ODD_PATHS {
        let o = python(&[os("ro-uri"), os(p)]);
        assert_eq!(
            String::from_utf8(o.stdout).unwrap(),
            format!("{}\n", pc::ro_uri(Path::new(p)))
        );
    }
}

fn store_in(dir: &Path, wal: bool) -> PathBuf {
    let db = dir.join("pulse store é?#.sqlite");
    let c = Connection::open(&db).unwrap();
    let fx = golden().join("vectors/08-db/fixture.sql");
    let mut sql = fs::read_to_string(fx).unwrap();
    if wal {
        sql = format!("PRAGMA journal_mode=WAL;\n{sql}");
    }
    c.execute_batch(&sql).unwrap();
    db
}

const WRITES: [&str; 5] = [
    "INSERT INTO archive_events (ts, source, payload) VALUES ('2026-11-13', 'x', 'y')",
    "UPDATE memory_traces SET snippet = 'changed'",
    "DELETE FROM memory_traces",
    "CREATE TABLE sneaky (x)",
    "PRAGMA user_version = 7",
];

#[test]
fn rust_open_ro_refuses_every_write() {
    for wal in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let db = store_in(tmp.path(), wal);
        let before = snapshot(&db);
        let conn = pc::open_ro(&db).unwrap();
        assert!(conn.is_readonly(rusqlite::DatabaseName::Main).unwrap());
        let q: i64 = conn
            .query_row("PRAGMA query_only", [], |r| r.get(0))
            .unwrap();
        assert_eq!(q, 1);
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM memory_traces", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2, "odd path still reads the right rows");
        for w in WRITES {
            assert!(conn.execute_batch(w).is_err(), "write succeeded: {w}");
        }
        // Even with query_only switched off, mode=ro still refuses.
        conn.execute_batch("PRAGMA query_only = OFF").unwrap();
        assert!(conn.execute_batch(WRITES[1]).is_err());
        drop(conn);
        assert_untouched(&db, &before);
    }
}

#[test]
fn open_ro_does_not_create_a_missing_store() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("absent.sqlite");
    assert!(pc::open_ro(&db).is_err());
    assert!(!db.exists());
    let o = python(&[
        os("canon"),
        os("--db"),
        db.as_os_str(),
        os("--exported-at"),
        os("2026-11-13T12:00:00Z"),
        os("--host"),
        os("laptop-1"),
    ]);
    assert_eq!(o.status.code(), Some(2), "{o:?}");
    assert!(o.stdout.is_empty());
    assert!(!db.exists());
}

#[test]
fn python_open_ro_refuses_every_write() {
    for wal in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let db = store_in(tmp.path(), wal);
        let before = snapshot(&db);
        let code = "import json, sqlite3\n\
            c = m.open_ro(sys.argv[2])\n\
            assert c.execute('PRAGMA query_only').fetchone()[0] == 1\n\
            assert c.execute('SELECT COUNT(*) FROM memory_traces').fetchone()[0] == 2\n\
            for w in json.loads(sys.argv[3]):\n\
            \x20   try:\n\
            \x20       c.execute(w); c.commit()\n\
            \x20   except sqlite3.Error:\n\
            \x20       continue\n\
            \x20   raise SystemExit('write succeeded: ' + w)\n\
            c.execute('PRAGMA query_only=OFF')\n\
            try:\n\
            \x20   c.execute(\"UPDATE memory_traces SET snippet='x'\"); c.commit()\n\
            \x20   raise SystemExit('mode=ro let a write through')\n\
            except sqlite3.OperationalError:\n\
            \x20   pass\n\
            c.close()\n\
            print('refused')";
        let o = python_eval(
            code,
            &[db.as_os_str(), os(&serde_json::to_string(&WRITES).unwrap())],
        );
        assert_eq!(String::from_utf8_lossy(&o.stdout), "refused\n", "{o:?}");
        assert_untouched(&db, &before);
    }
}

#[test]
fn python_checker_opens_with_uri_and_query_only() {
    let src = fs::read_to_string(checker()).unwrap();
    assert!(src.contains("sqlite3.connect(ro_uri(path), uri=True)"));
    assert!(src.contains("PRAGMA query_only=ON"));
    let rs = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/pulse_core.rs"))
        .unwrap();
    let body = &rs[rs.find("pub fn open_ro(").unwrap()..];
    let body = &body[..body.find("\n}\n").unwrap()];
    assert!(body.contains("ro_uri(path)") && body.contains("SQLITE_OPEN_URI"));
    assert!(body.contains("PRAGMA query_only = ON"));
}

// ---------------------------------------------------------------- evidence on admit

#[test]
fn admit_writes_the_content_hash_python_expects() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("pulse.sqlite");
    let fx = golden().join("vectors/08-db/fixture.sql");
    Connection::open(&db)
        .unwrap()
        .execute_batch(&fs::read_to_string(fx).unwrap())
        .unwrap();
    let _w = setup_writer();
    KernelWriter::open_pulse(&db)
        .unwrap()
        .admit(
            &ps::AdmitRequest {
                key: "demo/admitted".into(),
                snippet: "checked by a tool".into(),
                source: "fixture".into(),
                time: "2026-11-13T12:00:00+00:00".into(),
                status: "tool-verified".into(),
                kind: "fact".into(),
                lock: "open".into(),
                supersedes: None,
                reason: None,
                writer: "laptop-1".into(),
            },
            &ps::SupersedeFault::default(),
        )
        .unwrap();
    let conn = pc::open_ro(&db).unwrap();
    let h: String = conn
        .query_row(
            "SELECT verify_hash FROM memory_traces WHERE nat_key = 'demo/admitted'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(h, canon::content_hash("checked by a tool"));
    // The whole store, including the admitted row, now passes the canonical check.
    let (doc, v) = canon::canonical_from_db_path(&db, "2026-11-13T12:00:00Z", "laptop-1").unwrap();
    assert!(doc.is_some(), "{v:?}");
    assert_eq!(v.verdict, "ok");
}

#[test]
fn tla_rules_map_to_tests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cfg = std::fs::read_to_string(root.join("docs/spec/tla/PulseTrace.cfg")).unwrap();
    let readme = std::fs::read_to_string(root.join("docs/spec/tla/README.md")).unwrap();
    let mut sources = String::new();
    for entry in std::fs::read_dir(root.join("crates/ensembly-kernel/tests")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            sources.push_str(&std::fs::read_to_string(path).unwrap());
        }
    }
    let rules: Vec<&str> = cfg
        .lines()
        .filter_map(|l| {
            l.strip_prefix("INVARIANT ")
                .or_else(|| l.strip_prefix("PROPERTY "))
        })
        .map(str::trim)
        .collect();
    assert_eq!(rules.len(), 8, "{rules:?}");
    for rule in rules {
        let row = readme
            .lines()
            .find(|l| l.starts_with(&format!("| `{rule}` |")))
            .unwrap_or_else(|| panic!("README has no row for {rule}"));
        let cell = row.rsplit('|').nth(1).unwrap().trim();
        if cell == "model only" || cell.starts_with("pending:") {
            continue;
        }
        for name in cell.split(", ") {
            let name = name.trim_matches('`');
            assert!(
                sources.contains(&format!("fn {name}()")),
                "{rule}: no test fn {name}"
            );
        }
    }
}

// ---------------------------------------------------------------- honest pulse check

/// Same store, same findings: Rust `pulse check` lists and Python `db-check` both
/// reproduce the golden, and neither writes.
#[test]
fn db_check_findings_match_goldens_in_rust_and_python() {
    let dirs = db_check_vectors();
    assert_eq!(dirs.len(), 2);
    for d in dirs {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("pulse.sqlite");
        Connection::open(&db)
            .unwrap()
            .execute_batch(&fs::read_to_string(d.join("fixture.sql")).unwrap())
            .unwrap();
        let before = snapshot(&db);
        let want = fs::read(d.join("db-check.json")).unwrap();

        let conn = pc::open_ro(&db).unwrap();
        let evidence = pc::unpinned_verified(&conn).unwrap();
        let supersession = pc::half_supersessions(&conn).unwrap();
        drop(conn);
        let warn = !(evidence.is_empty() && supersession.is_empty());
        let got = serde_json::json!({
            "evidence": evidence,
            "exit": if warn { 1 } else { 0 },
            "supersession": supersession,
            "verdict": if warn { "warn" } else { "ok" },
        });
        assert_eq!(
            canon::canonical_bytes(&got).unwrap(),
            want,
            "{}",
            d.display()
        );

        let py = python(&[os("db-check"), db.as_os_str()]);
        assert_eq!(py.stdout, want, "{}", d.display());
        assert_eq!(py.status.code(), Some(if warn { 1 } else { 0 }));
        assert_untouched(&db, &before);
    }
}

/// The two inputs that used to differ now give one code each, on both sides.
#[test]
fn number_edge_codes_agree() {
    for (raw, code, path) in [
        (
            &b"18446744073709551616"[..],
            "E_RANGE",
            "/traces/0/seen_count",
        ),
        (b"-18446744073709551616", "E_RANGE", "/traces/0/seen_count"),
        (b"9223372036854775808", "E_RANGE", "/traces/0/seen_count"),
        (b"-9223372036854775809", "E_RANGE", "/traces/0/seen_count"),
        (b"1E400", "E_JSON", ""),
        (b"-1e400", "E_JSON", ""),
        (b"1e-400", "E_FLOAT", "/traces/0/seen_count"),
        (b"1e20", "E_FLOAT", "/traces/0/seen_count"),
    ] {
        let base = fs::read(golden().join("vectors/32-unsafe-int/input.json")).unwrap();
        let text = String::from_utf8(base)
            .unwrap()
            .replace("9007199254740992", std::str::from_utf8(raw).unwrap());
        let (_, v) = canon::check_bytes(text.as_bytes());
        let want = vec![(path.to_string(), code.to_string())];
        let got: Vec<_> = v
            .errors
            .iter()
            .map(|i| (i.path.clone(), i.code.clone()))
            .collect();
        assert_eq!(got, want, "rust {}", String::from_utf8_lossy(raw));
        let tmp = tempfile::NamedTempFile::new().unwrap();
        fs::write(tmp.path(), &text).unwrap();
        let py = python(&[os("check"), tmp.path().as_os_str()]);
        assert_eq!(
            py.stdout,
            canon::verdict_bytes(&v),
            "python {}",
            String::from_utf8_lossy(raw)
        );
    }
}

/// A store reached through a symlink opens read-only like any other: writes fail and
/// the target's bytes do not change. A dangling link is an error and creates nothing.
#[test]
fn open_ro_through_a_symlink_refuses_writes_and_creates_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("real.sqlite");
    Connection::open(&target)
        .unwrap()
        .execute_batch("CREATE TABLE t (x INTEGER); INSERT INTO t VALUES (1);")
        .unwrap();
    let link = tmp.path().join("link.sqlite");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let before = sha(&fs::read(&target).unwrap());

    let conn = pc::open_ro(&link).unwrap();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
    for sql in [
        "INSERT INTO t VALUES (2)",
        "DELETE FROM t",
        "CREATE TABLE u (y INTEGER)",
        "PRAGMA user_version = 9",
    ] {
        assert!(conn.execute_batch(sql).is_err(), "{sql} must fail");
    }
    drop(conn);
    assert_eq!(sha(&fs::read(&target).unwrap()), before);
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());

    let dangling = tmp.path().join("dangling.sqlite");
    let missing = tmp.path().join("missing.sqlite");
    std::os::unix::fs::symlink(&missing, &dangling).unwrap();
    assert!(pc::open_ro(&dangling).is_err());
    assert!(!missing.exists(), "open_ro created the link target");
}
