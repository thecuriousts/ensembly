use std::fs::{self, OpenOptions};
use std::io::{copy, Read, Write};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

#[cfg(target_os = "linux")]
const O_NOFOLLOW: i32 = 0o100000;
#[cfg(all(unix, not(target_os = "linux")))]
const O_NOFOLLOW: i32 = 0x00200000;

pub fn copy_nofollow(src: &Path, dest: &Path) -> Result<(), String> {
    let data = fs::read(src).map_err(|e| e.to_string())?;
    write_nofollow(dest, &data, dest.exists())
}

pub fn write_nofollow(dest: &Path, data: &[u8], allow_truncate: bool) -> Result<(), String> {
    if let Ok(meta) = fs::symlink_metadata(dest) {
        if meta.file_type().is_symlink() {
            return Err(format!("refusing symlink target {}", dest.display()));
        }
    }
    if dest.exists() && !allow_truncate {
        return Err(format!(
            "refusing overwrite {}; pass --merge",
            dest.display()
        ));
    }
    let mut opts = OpenOptions::new();
    opts.write(true);
    #[cfg(unix)]
    opts.custom_flags(O_NOFOLLOW);
    if dest.exists() {
        opts.truncate(true);
    } else {
        opts.create_new(true);
    }
    let mut file = opts
        .open(dest)
        .map_err(|e| format!("open {}: {e}", dest.display()))?;
    file.write_all(data).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn copy_reader_nofollow(
    dest: &Path,
    reader: &mut impl Read,
    allow_truncate: bool,
) -> Result<(), String> {
    if let Ok(meta) = fs::symlink_metadata(dest) {
        if meta.file_type().is_symlink() {
            return Err(format!("refusing symlink target {}", dest.display()));
        }
    }
    if dest.exists() && !allow_truncate {
        return Err(format!(
            "refusing overwrite {}; pass --merge",
            dest.display()
        ));
    }
    let mut opts = OpenOptions::new();
    opts.write(true);
    #[cfg(unix)]
    opts.custom_flags(O_NOFOLLOW);
    if dest.exists() {
        opts.truncate(true);
    } else {
        opts.create_new(true);
    }
    let mut file = opts
        .open(dest)
        .map_err(|e| format!("open {}: {e}", dest.display()))?;
    copy(reader, &mut file).map_err(|e| e.to_string())?;
    Ok(())
}
