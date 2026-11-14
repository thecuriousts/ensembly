//! Restore-style copies must refuse symlink endpoints (memory JSON and sources).

use ensembly_bundle::{copy_nofollow, restore_file_with_merge};
use std::fs;
use std::io::Write;
use tempfile::tempdir;

#[cfg(unix)]
#[test]
fn copy_nofollow_refuses_symlink_source_and_destination() {
    let dir = tempdir().unwrap();
    let real = dir.path().join("real.json");
    fs::write(&real, br#"{"ok":true}"#).unwrap();
    let link_src = dir.path().join("mem-link.json");
    std::os::unix::fs::symlink(&real, &link_src).unwrap();
    let dest = dir.path().join("dest.json");
    fs::write(&dest, b"{}").unwrap();
    let err_src = copy_nofollow(&link_src, &dest).unwrap_err();
    assert!(err_src.contains("symlink source"), "{err_src}");

    let payload = dir.path().join("payload.json");
    fs::write(&payload, br#"{"from":"bundle"}"#).unwrap();
    let link_dest = dir.path().join("dest-link.json");
    std::os::unix::fs::symlink(dir.path().join("outside.json"), &link_dest).unwrap();
    let err_dest = copy_nofollow(&payload, &link_dest).unwrap_err();
    assert!(err_dest.contains("symlink target"), "{err_dest}");
}

#[cfg(unix)]
#[test]
fn restore_file_with_merge_refuses_symlink_source_and_destination() {
    let dir = tempdir().unwrap();
    let bundle = dir.path().join("bundle/shared/docs/data.txt");
    fs::create_dir_all(bundle.parent().unwrap()).unwrap();
    fs::write(&bundle, b"bundle-bytes").unwrap();
    let real_src = dir.path().join("real-src.txt");
    fs::write(&real_src, b"hidden").unwrap();
    let symlink_src = dir.path().join("link-src.txt");
    std::os::unix::fs::symlink(&real_src, &symlink_src).unwrap();
    let err = restore_file_with_merge(&symlink_src, &dir.path().join("out.txt"), false, dir.path())
        .unwrap_err();
    assert!(err.contains("symlink source"), "{err}");

    let outside = dir.path().join("outside.txt");
    fs::write(&outside, b"keep").unwrap();
    let link_dest = dir.path().join("restore/mem-link.json");
    fs::create_dir_all(link_dest.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&outside, &link_dest).unwrap();
    let err_dest = restore_file_with_merge(&bundle, &link_dest, false, dir.path()).unwrap_err();
    assert!(
        err_dest.contains("symlink target") || err_dest.contains("symlink"),
        "{err_dest}"
    );
}

#[test]
fn copy_nofollow_streams_large_file_without_read_all() {
    const CHUNK: usize = 1024 * 1024;
    const CHUNKS: usize = 200;
    let dir = tempdir().unwrap();
    let src = dir.path().join("large.bin");
    let mut f = fs::File::create(&src).unwrap();
    let block = vec![0xABu8; CHUNK];
    for _ in 0..CHUNKS {
        f.write_all(&block).unwrap();
    }
    drop(f);
    let dest = dir.path().join("copy.bin");
    copy_nofollow(&src, &dest).unwrap();
    assert_eq!(
        fs::metadata(&src).unwrap().len(),
        fs::metadata(&dest).unwrap().len()
    );
}
