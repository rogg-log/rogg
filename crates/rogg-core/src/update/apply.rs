//! Verifying a download, unpacking it and swapping it in for the installed rogg, with a rollback if the swap fails.
//! Nothing here touches the network.

use super::{Install, UpdateError};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

const NEW_SUFFIX: &str = ".rogg-new";
const OLD_SUFFIX: &str = ".rogg-old";
const WINDOWS_OLD_SUFFIX: &str = ".old";

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> UpdateError + '_ {
    move |source| UpdateError::Io { path: path.to_path_buf(), source }
}

/// `<path><suffix>`, beside `path`.
fn beside(path: &Path, suffix: &str) -> PathBuf {
    let mut name: OsString = path.file_name().map(OsString::from).unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

/// Removes a file, a symlink or a whole directory; absent is fine.
fn remove_any(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Checks the SHA-256 of `file` against `sha256_hex` (case-insensitive).
pub fn verify(file: &Path, sha256_hex: &str) -> Result<(), UpdateError> {
    let mut f = File::open(file).map_err(io_err(file))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(io_err(file))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if digest.eq_ignore_ascii_case(sha256_hex.trim()) { Ok(()) } else { Err(UpdateError::Checksum) }
}

fn unsafe_path(path: &Path) -> bool {
    path.components().any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
}

fn unpack_err(e: impl std::fmt::Display) -> UpdateError {
    UpdateError::Unpack(e.to_string())
}

fn unpack_tar_gz(archive: &Path, dir: &Path) -> Result<(), UpdateError> {
    let file = File::open(archive).map_err(io_err(archive))?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    for entry in tar.entries().map_err(unpack_err)? {
        let mut entry = entry.map_err(unpack_err)?;
        let path = entry.path().map_err(unpack_err)?.into_owned();
        if unsafe_path(&path) {
            return Err(UpdateError::Unpack(format!("entry {} escapes the folder", path.display())));
        }
        let kind = entry.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() {
            let link = entry.link_name().map_err(unpack_err)?;
            if link.as_deref().is_none_or(unsafe_path) {
                return Err(UpdateError::Unpack(format!("link {} points outside the folder", path.display())));
            }
        }
        if !entry.unpack_in(dir).map_err(unpack_err)? {
            return Err(UpdateError::Unpack(format!("entry {} was refused", path.display())));
        }
    }
    Ok(())
}

fn unpack_zip(archive: &Path, dir: &Path) -> Result<(), UpdateError> {
    let file = File::open(archive).map_err(io_err(archive))?;
    let mut zip = zip::ZipArchive::new(file).map_err(unpack_err)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(unpack_err)?;
        let Some(rel) = entry.enclosed_name() else {
            return Err(UpdateError::Unpack(format!("entry {} escapes the folder", entry.name())));
        };
        let out = dir.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(unpack_err)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(unpack_err)?;
        }
        let mut f = File::create(&out).map_err(unpack_err)?;
        io::copy(&mut entry, &mut f).map_err(unpack_err)?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&out, fs::Permissions::from_mode(mode & 0o777)).map_err(unpack_err)?;
        }
    }
    Ok(())
}

/// Unpacks `archive` (`.zip`, otherwise `.tar.gz`) into `dir` and returns the item to install: `rogg`, `rogg.exe` or
/// `rogg.app` at the archive root, according to `install`. Entries that would land outside `dir` fail the unpack.
pub fn stage(archive: &Path, dir: &Path, install: &Install) -> Result<PathBuf, UpdateError> {
    let item: &'static str = match install {
        Install::App { .. } => "rogg.app",
        Install::Binary { path } if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) => "rogg.exe",
        Install::Binary { .. } => "rogg",
        Install::Unsupported => return Err(UpdateError::Unpack("this install can't be updated".into())),
    };
    fs::create_dir_all(dir).map_err(io_err(dir))?;
    if archive.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        unpack_zip(archive, dir)?;
    } else {
        unpack_tar_gz(archive, dir)?;
    }
    let staged = dir.join(item);
    // A link would be followed once the item is moved beside the target, so only real files and folders count.
    let is_real = |p: &Path, dir: bool| {
        fs::symlink_metadata(p).is_ok_and(|m| if dir { m.file_type().is_dir() } else { m.file_type().is_file() })
    };
    let ok = match install {
        Install::App { .. } => is_real(&staged, true) && is_real(&staged.join("Contents/MacOS/rogg"), false),
        _ => is_real(&staged, false),
    };
    if !ok {
        return Err(UpdateError::MissingItem(item));
    }
    Ok(staged)
}

/// How the old version is dealt with once the new one is in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplaceStyle {
    /// The old version is deleted.
    Unix,
    /// A running exe can be renamed but not deleted, so it stays beside the target as `<target>.old`.
    Windows,
}

impl ReplaceStyle {
    pub fn current() -> ReplaceStyle {
        if cfg!(windows) { ReplaceStyle::Windows } else { ReplaceStyle::Unix }
    }
}

/// Test hook: makes one step of [`apply_with_fault`] fail.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    None,
    /// The final rename of `<target>.rogg-new` onto the target fails.
    SwapIn,
}

fn is_app_name(target: &Path) -> bool {
    target.extension().is_some_and(|e| e.eq_ignore_ascii_case("app"))
}

fn rogg_name(target: &Path) -> bool {
    let Some(name) = target.file_name().and_then(|n| n.to_str()) else { return false };
    name.eq_ignore_ascii_case("rogg") || name.eq_ignore_ascii_case("rogg.exe") || is_app_name(target)
}

/// Only rogg itself may be replaced: a binary named `rogg` / `rogg.exe`, or an app bundle (any `<name>.app`, e.g.
/// Finder's `rogg 2.app`) that holds `Contents/MacOS/rogg`.
pub fn is_rogg_target(target: &Path) -> bool {
    if !rogg_name(target) {
        return false;
    }
    !is_app_name(target)
        || fs::symlink_metadata(target.join("Contents/MacOS/rogg")).is_ok_and(|m| m.file_type().is_file())
}

/// A failed update can leave the only copy at `<target>.rogg-old` with the target gone; put it back.
fn restore_old(target: &Path) -> Result<(), UpdateError> {
    let old = beside(target, OLD_SUFFIX);
    if fs::symlink_metadata(target).is_err() && fs::symlink_metadata(&old).is_ok() {
        fs::rename(&old, target).map_err(io_err(&old))?;
    }
    Ok(())
}

pub fn apply(staged: &Path, target: &Path, style: ReplaceStyle) -> Result<(), UpdateError> {
    apply_with_fault(staged, target, style, Fault::None)
}

#[cfg(unix)]
fn make_executable(item: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let binary = if fs::symlink_metadata(item)?.is_dir() { item.join("Contents/MacOS/rogg") } else { item.to_path_buf() };
    // Never chmod through a link: it could point anywhere.
    if !fs::symlink_metadata(&binary)?.file_type().is_file() {
        return Err(io::Error::other("the new version is not a regular file"));
    }
    fs::set_permissions(binary, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_executable(_item: &Path) -> io::Result<()> {
    Ok(())
}

/// Replaces `target` with `staged` (consumed). The old version is renamed aside first and put back if the new one
/// can't be moved in, so a failure leaves the previous version in place.
#[doc(hidden)]
pub fn apply_with_fault(staged: &Path, target: &Path, style: ReplaceStyle, fault: Fault) -> Result<(), UpdateError> {
    if !rogg_name(target) {
        return Err(UpdateError::NotRogg(target.to_path_buf()));
    }
    restore_old(target)?;
    if !is_rogg_target(target) {
        return Err(UpdateError::NotRogg(target.to_path_buf()));
    }
    let new = beside(target, NEW_SUFFIX);
    let old = beside(target, OLD_SUFFIX);
    // Leftovers of an earlier attempt would make the renames below fail. The target exists here, so `old` is stale.
    remove_any(&new).map_err(io_err(&new))?;
    remove_any(&old).map_err(io_err(&old))?;

    if let Err(e) = move_into(staged, &new).and_then(|()| make_executable(&new)) {
        let _ = remove_any(&new);
        return Err(UpdateError::Io { path: new, source: e });
    }
    if let Err(source) = fs::rename(target, &old) {
        let _ = remove_any(&new);
        return Err(UpdateError::Replace { rolled_back: true, old: target.to_path_buf(), source });
    }
    let swap = match fault {
        Fault::SwapIn => Err(io::Error::other("injected failure")),
        Fault::None => fs::rename(&new, target),
    };
    if let Err(source) = swap {
        let rolled_back = fs::rename(&old, target).is_ok();
        if rolled_back {
            let _ = remove_any(&new);
        }
        // Where the previous version is now.
        let old = if rolled_back { target.to_path_buf() } else { old };
        return Err(UpdateError::Replace { rolled_back, old, source });
    }
    // The new version is live; failing to tidy up the old one is not a failure (sweep gets it next start).
    match style {
        ReplaceStyle::Unix => {
            let _ = remove_any(&old);
        }
        ReplaceStyle::Windows => {
            let aside = beside(target, WINDOWS_OLD_SUFFIX);
            let _ = remove_any(&aside);
            let _ = fs::rename(&old, &aside);
        }
    }
    Ok(())
}

/// Moves a file or tree: a rename, or a copy and delete when `dst` is on another volume.
pub fn move_into(src: &Path, dst: &Path) -> io::Result<()> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
            if let Err(e) = copy_tree(src, dst) {
                let _ = remove_any(dst);
                return Err(e);
            }
            remove_any(src)
        }
        Err(e) => Err(e),
    }
}

/// Copies a file or a whole tree (permissions kept, symlinks recreated on Unix).
pub fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(src)?;
    if meta.is_dir() {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            copy_tree(&entry.path(), &dst.join(entry.file_name()))?;
        }
        fs::set_permissions(dst, meta.permissions())
    } else if meta.is_symlink() {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(fs::read_link(src)?, dst)
        }
        #[cfg(not(unix))]
        {
            fs::copy(src, dst).map(|_| ())
        }
    } else {
        fs::copy(src, dst).map(|_| ())
    }
}

/// Removes what an update leaves behind (best effort): the names beside the target and the staging folder.
pub fn sweep(target: Option<&Path>, update_dir: &Path) {
    if let Some(target) = target {
        // With the target gone, `.rogg-old` is the only copy of the program: restore it rather than delete it.
        let _ = restore_old(target);
        for suffix in [NEW_SUFFIX, OLD_SUFFIX, WINDOWS_OLD_SUFFIX] {
            let _ = remove_any(&beside(target, suffix));
        }
    }
    let _ = remove_any(update_dir);
}

/// Whether files can be created in `dir` (so the update needs no admin rights).
pub fn can_write(dir: &Path) -> bool {
    let probe = dir.join(format!(".rogg-write-probe-{}", std::process::id()));
    match fs::OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}
