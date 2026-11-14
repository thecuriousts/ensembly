//! Hostile tar inputs for bundle join unpack (review matrix from #86).

use chrono::{TimeZone, Utc};
use ensembly_bundle::{
    export_bundle, hash_file, join_bundle, pack_bundle, unpack_tar_to_bundle, BackupSource,
    BackupSourceKind, EnsemblyStorePaths, ExportOpts, JoinOpts, PackOpts, DEFAULT_MAX_FILE_MB,
};
use std::fs;
use std::path::Path;
use tar::{Builder, EntryType, Header};
use tempfile::tempdir;

fn append_gnu_longname_only(writer: &mut Vec<u8>, payload: &[u8]) -> Result<(), String> {
    let mut b = Builder::new(writer);
    let mut long_header = Header::new_gnu();
    long_header
        .set_path("././@LongLink")
        .map_err(|e| e.to_string())?;
    long_header.set_size(payload.len() as u64);
    long_header.set_entry_type(EntryType::GNULongName);
    long_header.set_cksum();
    b.append(&long_header, payload).map_err(|e| e.to_string())?;
    b.into_inner().map_err(|e| e.to_string())?;
    Ok(())
}

fn append_gnu_longname_regular(
    writer: &mut Vec<u8>,
    payload: &[u8],
    short_name: &str,
    body: &[u8],
) -> Result<(), String> {
    let mut b = Builder::new(writer);
    let mut long_header = Header::new_gnu();
    long_header
        .set_path("././@LongLink")
        .map_err(|e| e.to_string())?;
    long_header.set_size(payload.len() as u64);
    long_header.set_entry_type(EntryType::GNULongName);
    long_header.set_cksum();
    b.append(&long_header, payload).map_err(|e| e.to_string())?;
    let mut file_header = Header::new_gnu();
    file_header
        .set_path(short_name)
        .map_err(|e| e.to_string())?;
    file_header.set_size(body.len() as u64);
    file_header.set_entry_type(EntryType::Regular);
    file_header.set_cksum();
    b.append(&file_header, body).map_err(|e| e.to_string())?;
    b.into_inner().map_err(|e| e.to_string())?;
    Ok(())
}

fn finish_tar(raw: &mut Vec<u8>) {
    let pad = 512 - (raw.len() % 512);
    if pad < 512 {
        raw.extend(std::iter::repeat_n(0u8, pad));
    }
    raw.extend([0u8; 1024]);
}

fn write_crafted_tar(path: &Path, build: impl FnOnce(&mut Vec<u8>) -> Result<(), String>) {
    let mut raw = Vec::new();
    build(&mut raw).expect("craft tar");
    finish_tar(&mut raw);
    fs::write(path, raw).unwrap();
}

fn synthetic_ops_bytes() -> Vec<u8> {
    let life = serde_json::json!({"graph": {"nodes": {}, "edges": []}});
    let bundle = serde_json::json!({
        "format": "ensembly-ops-bundle-v1",
        "schema_version": 1,
        "exported_at": "2026-01-01T00:00:00Z",
        "kv": [{"key": "life_state", "value": life.to_string(), "updated_at": "2026-01-01T00:00:00Z"}]
    });
    serde_json::to_vec(&bundle).unwrap()
}

fn export_with_extra(
    dir: &Path,
    name: &str,
    extra: impl FnOnce(&std::path::Path),
) -> std::path::PathBuf {
    let ops_tmp = dir.join("ops.json");
    fs::write(&ops_tmp, synthetic_ops_bytes()).unwrap();
    let shared = dir.join("shared-src");
    fs::create_dir_all(&shared).unwrap();
    fs::write(shared.join("note.md"), b"hello").unwrap();
    extra(&shared);
    let out = dir.join(name);
    export_bundle(ExportOpts {
        out: out.clone(),
        force_new_subdir: false,
        max_file_mb: DEFAULT_MAX_FILE_MB,
        ensembly_version: "test".into(),
        created_at: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
        stores: EnsemblyStorePaths {
            ops_json: Some(ops_tmp),
            memory_json: None,
            ledger_jsonl: None,
        },
        user_sources: vec![BackupSource {
            name: "docs".into(),
            section: "shared".into(),
            path: shared,
            kind: BackupSourceKind::Dir,
            include: vec!["**/*".into()],
            exclude: vec![],
            force_include: vec![],
            line: 1,
        }],
    })
    .unwrap();
    out
}

#[test]
fn pack_join_boundary_name_lengths_101_110_255() {
    for len in [101_usize, 110, 255] {
        let dir = tempdir().unwrap();
        let name = "n".repeat(len);
        let bundle = export_with_extra(dir.path(), &format!("bundle-{len}"), |shared| {
            fs::write(shared.join(&name), format!("len-{len}").as_bytes()).unwrap();
        });
        let parts_dir = dir.path().join("parts");
        let joined = dir.path().join("joined");
        pack_bundle(PackOpts {
            bundle_dir: bundle.clone(),
            out: parts_dir.clone(),
            split_mb: 32,
        })
        .unwrap();
        join_bundle(JoinOpts {
            parts_json: parts_dir.join(format!("bundle-{len}.parts.json")),
            out: joined.clone(),
        })
        .unwrap();
        let rel = format!("shared/docs/{name}");
        assert_eq!(
            fs::read(bundle.join(&rel)).unwrap(),
            fs::read(joined.join(&rel)).unwrap()
        );
    }
}

#[test]
fn unpack_refuses_absolute_longname() {
    let dir = tempdir().unwrap();
    let tar = dir.path().join("abs.tar");
    write_crafted_tar(&tar, |raw| {
        let abs = format!("/{}", "z".repeat(120));
        let payload = format!("{abs}\0");
        append_gnu_longname_regular(raw, payload.as_bytes(), "x", b"x")
    });
    let err = unpack_tar_to_bundle(&tar, &dir.path().join("out")).unwrap_err();
    assert!(
        err.contains("absolute") || err.contains("manifest") || err.contains("relative"),
        "{err}"
    );
}

#[test]
fn unpack_refuses_nul_in_longname() {
    let dir = tempdir().unwrap();
    let tar = dir.path().join("nul.tar");
    write_crafted_tar(&tar, |raw| {
        let payload: Vec<u8> = b"sources/shared/a\0evil".to_vec();
        append_gnu_longname_regular(raw, &payload, "x", b"x")
    });
    let err = unpack_tar_to_bundle(&tar, &dir.path().join("out")).unwrap_err();
    assert!(err.contains("NUL") || err.contains("manifest"), "{err}");
}

#[test]
fn unpack_refuses_long_linkname_symlink_and_hardlink() {
    for link_type in [EntryType::Symlink, EntryType::Link] {
        let dir = tempdir().unwrap();
        let tar = dir.path().join("link.tar");
        write_crafted_tar(&tar, |raw| {
            let mut b = Builder::new(raw);
            let mut h = Header::new_gnu();
            h.set_entry_type(link_type);
            h.set_size(0);
            let target = format!("other/{}", "t".repeat(120));
            b.append_link(&mut h, "entry.txt", &target)
                .map_err(|e| e.to_string())?;
            b.into_inner().map_err(|e| e.to_string())?;
            Ok(())
        });
        let err = unpack_tar_to_bundle(&tar, &dir.path().join("out")).unwrap_err();
        assert!(
            err.contains("symlink") || err.contains("hardlink") || err.contains("link"),
            "{err}"
        );
    }
}

#[test]
fn unpack_refuses_stray_pax_local() {
    let dir = tempdir().unwrap();
    let tar = dir.path().join("pax.tar");
    write_crafted_tar(&tar, |raw| {
        let mut b = Builder::new(raw);
        let mut h = Header::new_gnu();
        h.set_path("././@PaxHeader/only")
            .map_err(|e| e.to_string())?;
        h.set_size(4);
        h.set_entry_type(EntryType::XHeader);
        h.set_cksum();
        b.append(&h, &b"blah"[..]).map_err(|e| e.to_string())?;
        b.into_inner().map_err(|e| e.to_string())?;
        Ok(())
    });
    let err = unpack_tar_to_bundle(&tar, &dir.path().join("out")).unwrap_err();
    assert!(
        err.contains("stray") || err.contains("future member") || err.contains("metadata"),
        "{err}"
    );
}

#[test]
fn unpack_refuses_orphan_longlink() {
    let dir = tempdir().unwrap();
    let tar = dir.path().join("orphan.tar");
    write_crafted_tar(&tar, |raw| {
        append_gnu_longname_only(raw, b"sources/shared/ok.txt\0")
    });
    let err = unpack_tar_to_bundle(&tar, &dir.path().join("out")).unwrap_err();
    assert!(
        err.contains("future member") || err.contains("stray") || err.contains("metadata"),
        "{err}"
    );
}

#[test]
fn unpack_refuses_two_consecutive_longlinks() {
    let dir = tempdir().unwrap();
    let tar = dir.path().join("dbl-l.tar");
    write_crafted_tar(&tar, |raw| {
        append_gnu_longname_only(raw, b"sources/shared/a\0")?;
        append_gnu_longname_regular(raw, b"sources/shared/b\0", "y", b"y")
    });
    let err = unpack_tar_to_bundle(&tar, &dir.path().join("out")).unwrap_err();
    assert!(
        err.contains("future member")
            || err.contains("stray")
            || err.contains("metadata")
            || err.contains("duplicate"),
        "{err}"
    );
}

#[cfg(unix)]
#[test]
fn unpack_refuses_symlink_parent_under_out() {
    let dir = tempdir().unwrap();
    let tar = dir.path().join("ok.tar");
    write_crafted_tar(&tar, |raw| {
        let mut b = Builder::new(raw);
        let mut h = Header::new_gnu();
        h.set_size(3);
        b.append_data(&mut h, "sources/shared/safe.txt", &b"ok"[..])
            .map_err(|e| e.to_string())?;
        b.into_inner().map_err(|e| e.to_string())?;
        Ok(())
    });
    let out = dir.path().join("out");
    fs::create_dir_all(&out).unwrap();
    let trap = out.join("sources");
    fs::remove_dir_all(&trap).ok();
    let outside = dir.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, &trap).unwrap();
    let err = unpack_tar_to_bundle(&tar, &out).unwrap_err();
    assert!(err.contains("symlink"), "{err}");
}

#[test]
fn unpack_refuses_duplicate_paths() {
    let dir = tempdir().unwrap();
    let tar = dir.path().join("dup.tar");
    write_crafted_tar(&tar, |raw| {
        let mut b = Builder::new(raw);
        let mut h = Header::new_gnu();
        h.set_size(1);
        b.append_data(&mut h, "sources/shared/dup.txt", &b"a"[..])
            .map_err(|e| e.to_string())?;
        let mut h2 = Header::new_gnu();
        h2.set_size(1);
        b.append_data(&mut h2, "sources/shared/dup.txt", &b"b"[..])
            .map_err(|e| e.to_string())?;
        b.into_inner().map_err(|e| e.to_string())?;
        Ok(())
    });
    let err = unpack_tar_to_bundle(&tar, &dir.path().join("out")).unwrap_err();
    assert!(err.contains("duplicate tar path"), "{err}");
    assert!(!err.contains("pass --merge"), "{err}");
}

#[test]
fn join_rollback_after_duplicate_tar_path() {
    let dir = tempdir().unwrap();
    let tar = dir.path().join("dup-only.tar");
    write_crafted_tar(&tar, |raw| {
        let mut b = Builder::new(raw);
        let mut h = Header::new_gnu();
        h.set_size(2);
        b.append_data(&mut h, "MANIFEST.json", &b"[]{}"[..])
            .map_err(|e| e.to_string())?;
        let mut h2 = Header::new_gnu();
        h2.set_size(2);
        b.append_data(&mut h2, "MANIFEST.json", &b"xx"[..])
            .map_err(|e| e.to_string())?;
        b.into_inner().map_err(|e| e.to_string())?;
        Ok(())
    });
    let parts_dir = dir.path().join("parts");
    fs::create_dir_all(&parts_dir).unwrap();
    let part_name = "evil.tar.part001";
    fs::copy(&tar, parts_dir.join(&part_name)).unwrap();
    let parts_json = parts_dir.join("evil.parts.json");
    let hashed = hash_file(&tar).unwrap();
    let manifest = serde_json::json!({
        "parts_format": "ensembly-portable-bundle-parts-v1",
        "bundle_name": "evil",
        "split_mb": 16,
        "part_count": 1,
        "total_sha256": hashed.sha256,
        "parts": [{
            "index": 1,
            "file": part_name,
            "sha256": hashed.sha256,
            "bytes": hashed.bytes,
        }]
    });
    fs::write(&parts_json, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    let target = dir.path().join("joined");
    let err = join_bundle(JoinOpts {
        parts_json,
        out: target.clone(),
    })
    .unwrap_err();
    assert!(err.contains("duplicate tar path"), "{err}");
    assert!(!target.join("MANIFEST.json").exists());
}
