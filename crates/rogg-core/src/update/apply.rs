//! Verifying a download, unpacking it and swapping it in for the installed rogg, with a rollback if the swap fails.
//! Nothing here touches the network.

use super::{Install, UpdateError};
use crate::platform::Os;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read, Seek};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

const NEW_SUFFIX: &str = ".rogg-new";
const OLD_SUFFIX: &str = ".rogg-old";
const WINDOWS_OLD_SUFFIX: &str = ".old";
/// The elevated helper's own folder beside the target: `<target>.rogg-stage-<pid>`.
const STAGE_SUFFIX: &str = ".rogg-stage";

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
    let f = File::open(file).map_err(io_err(file))?;
    verify_reader(f, sha256_hex).map_err(|e| match e {
        UpdateError::Read(source) => UpdateError::Io { path: file.to_path_buf(), source },
        e => e,
    })
}

/// Checks the SHA-256 of everything `reader` yields against `sha256_hex` (case-insensitive).
pub fn verify_reader(mut reader: impl Read, sha256_hex: &str) -> Result<(), UpdateError> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(UpdateError::Read(e)),
        };
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

/// Permission bits an unpacked file never gets: set-id, sticky, and write for group and others.
const MODE_MASK: u32 = 0o7022;

fn unpack_tar_gz(archive: impl Read, dir: &Path) -> Result<(), UpdateError> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    // Set-id and sticky bits are dropped anyway (permissions aren't preserved); this drops group/other write too.
    tar.set_mask(MODE_MASK);
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

fn unpack_zip(archive: impl Read + Seek, dir: &Path) -> Result<(), UpdateError> {
    let mut zip = zip::ZipArchive::new(archive).map_err(unpack_err)?;
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
            fs::set_permissions(&out, fs::Permissions::from_mode(mode & 0o777 & !MODE_MASK)).map_err(unpack_err)?;
        }
    }
    Ok(())
}

/// The format of a release archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveKind {
    TarGz,
    Zip,
}

impl ArchiveKind {
    /// From the file name: `.tar.gz` or `.zip` (any case); anything else is `None`.
    pub fn of(path: &Path) -> Option<ArchiveKind> {
        let name = path.file_name()?.to_str()?.to_ascii_lowercase();
        if name.ends_with(".tar.gz") {
            Some(ArchiveKind::TarGz)
        } else if name.ends_with(".zip") {
            Some(ArchiveKind::Zip)
        } else {
            None
        }
    }
}

/// Unpacks `archive` (`.zip`, otherwise `.tar.gz`) into `dir` and returns the item to install; see [`stage_from`].
pub fn stage(archive: &Path, dir: &Path, install: &Install) -> Result<PathBuf, UpdateError> {
    let kind = ArchiveKind::of(archive).unwrap_or(ArchiveKind::TarGz);
    let file = File::open(archive).map_err(io_err(archive))?;
    stage_from(file, kind, dir, install)
}

/// Unpacks the `kind` archive read from `archive` into `dir` and returns the item to install: `rogg`, `rogg.exe` or
/// `rogg.app` at the archive root, according to `install`. Entries that would land outside `dir` fail the unpack;
/// unpacked files never get set-id bits or group/other write.
pub fn stage_from(
    archive: impl Read + Seek,
    kind: ArchiveKind,
    dir: &Path,
    install: &Install,
) -> Result<PathBuf, UpdateError> {
    let item: &'static str = match install {
        Install::App { .. } => "rogg.app",
        Install::Binary { path } if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) => "rogg.exe",
        Install::Binary { .. } => "rogg",
        Install::Unsupported => return Err(UpdateError::Unpack("this install can't be updated".into())),
    };
    fs::create_dir_all(dir).map_err(io_err(dir))?;
    match kind {
        ArchiveKind::Zip => unpack_zip(archive, dir)?,
        ArchiveKind::TarGz => unpack_tar_gz(archive, dir)?,
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

/// Test hook: makes steps of [`apply_with_fault`] fail.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    None,
    /// The final rename of `<target>.rogg-new` onto the target fails.
    SwapIn,
    /// A stale `<target>.rogg-old` can't be removed.
    StuckOld,
    /// Both.
    StuckOldAndSwapIn,
}

impl Fault {
    fn stuck_old(self) -> bool {
        matches!(self, Fault::StuckOld | Fault::StuckOldAndSwapIn)
    }

    fn swap_in(self) -> bool {
        matches!(self, Fault::SwapIn | Fault::StuckOldAndSwapIn)
    }
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

/// How many unique names [`old_aside`] tries.
const UNIQUE_OLD_TRIES: u32 = 100;

/// Where the old version is renamed aside: `<target>.rogg-old`, or, when a stale one is there and can't be removed
/// (a leftover owned by another user, or on Windows an old exe that is still running), `<target>.rogg-old-<pid>-<n>`
/// for this run, so a stuck leftover doesn't block every later update. Sweep removes those later.
fn old_aside(target: &Path, fault: Fault) -> Result<PathBuf, UpdateError> {
    let old = beside(target, OLD_SUFFIX);
    let removed = if fault.stuck_old() && fs::symlink_metadata(&old).is_ok() {
        Err(io::Error::other("injected failure"))
    } else {
        remove_any(&old)
    };
    let Err(stuck) = removed else { return Ok(old) };
    let pid = std::process::id();
    (0..UNIQUE_OLD_TRIES)
        .map(|n| beside(target, &format!("{OLD_SUFFIX}-{pid}-{n}")))
        .find(|p| fs::symlink_metadata(p).is_err_and(|e| e.kind() == io::ErrorKind::NotFound))
        .ok_or(UpdateError::Io { path: old, source: stuck })
}

/// `<target>.rogg-old-*`: old versions put aside under a name of their own by [`old_aside`].
fn unique_olds(target: &Path) -> Vec<PathBuf> {
    numbered_siblings(target, OLD_SUFFIX)
}

/// `<target>.rogg-stage-*`: the elevated helpers' folders ([`apply_archive`]).
fn stage_dirs(target: &Path) -> Vec<PathBuf> {
    numbered_siblings(target, STAGE_SUFFIX)
}

/// `<target><suffix>-*`, sorted.
fn numbered_siblings(target: &Path, suffix: &str) -> Vec<PathBuf> {
    let Some(name) = target.file_name() else { return Vec::new() };
    let mut prefix = name.to_os_string();
    prefix.push(suffix);
    prefix.push("-");
    let dir = target.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().as_encoded_bytes().starts_with(prefix.as_encoded_bytes()))
        .map(|e| target.with_file_name(e.file_name()))
        .collect();
    found.sort();
    found
}

/// A failed update can leave the only copy at `<target>.rogg-old` (or a unique `-<pid>-<n>` name) with the target
/// gone; put it back.
fn restore_old(target: &Path) -> Result<(), UpdateError> {
    if fs::symlink_metadata(target).is_ok() {
        return Ok(());
    }
    let mut candidates = std::iter::once(beside(target, OLD_SUFFIX)).chain(unique_olds(target));
    if let Some(old) = candidates.find(|old| fs::symlink_metadata(old).is_ok()) {
        fs::rename(&old, target).map_err(io_err(&old))?;
    }
    Ok(())
}

/// Replaces `target` with `staged`, which is moved into place (renamed, or copied across volumes) and so consumed.
/// The moved files keep their owner: the in-process update stages into the user's cache because the user owns the
/// install, and the elevated helper stages into a folder of its own ([`apply_archive`]).
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

/// [`apply`] with injected faults. The old version is renamed aside first and put back if the new one can't be moved
/// in, so a failure leaves the previous version in place.
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
    // Leftovers of an earlier attempt would make the renames below fail. The target exists here, so a `.rogg-old`
    // is stale.
    remove_any(&new).map_err(io_err(&new))?;
    let old = old_aside(target, fault)?;

    if let Err(e) = move_into(staged, &new).and_then(|()| make_executable(&new)) {
        let _ = remove_any(&new);
        return Err(UpdateError::Io { path: new, source: e });
    }
    if let Err(source) = fs::rename(target, &old) {
        let _ = remove_any(&new);
        return Err(UpdateError::Replace { rolled_back: true, old: target.to_path_buf(), source });
    }
    let swap = if fault.swap_in() { Err(io::Error::other("injected failure")) } else { fs::rename(&new, target) };
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

/// The largest download the elevated helper accepts; it holds the whole archive in memory.
pub const MAX_ARCHIVE: u64 = 512 * 1024 * 1024;

/// The elevated helper's install: checks the archive at `archive` against `sha256_hex`, unpacks it into a folder of
/// its own beside the target (`<target>.rogg-stage-<pid>`, which must not exist yet) and moves the item into place
/// with [`apply`]. The folder is removed afterwards, whatever happened.
///
/// The download sits in the user's cache, where the user can change it at any time, and the name could even be a
/// hard link to a file only root may read. So it is opened once (refusing a link or anything but a regular file) and
/// read once into memory (at most [`MAX_ARCHIVE`]); those bytes are hashed, and only once they match is anything
/// written: the archive is unpacked from memory. No byte of an unverified file ever reaches the disk. Everything
/// installed is created by this process, so with admin rights it gets the admin owner and the destination's
/// inherited permissions.
pub fn apply_archive(archive: &Path, sha256_hex: &str, target: &Path, style: ReplaceStyle) -> Result<(), UpdateError> {
    apply_archive_capped(archive, sha256_hex, target, style, MAX_ARCHIVE)
}

/// [`apply_archive`] with `cap` in place of [`MAX_ARCHIVE`] (tests).
#[doc(hidden)]
pub fn apply_archive_capped(
    archive: &Path,
    sha256_hex: &str,
    target: &Path,
    style: ReplaceStyle,
    cap: u64,
) -> Result<(), UpdateError> {
    if !rogg_name(target) {
        return Err(UpdateError::NotRogg(target.to_path_buf()));
    }
    let kind = ArchiveKind::of(archive)
        .ok_or_else(|| UpdateError::Unpack(format!("{} is not a .tar.gz or .zip", archive.display())))?;
    let install = if is_app_name(target) {
        Install::App { bundle: target.to_path_buf() }
    } else {
        Install::Binary { path: target.to_path_buf() }
    };
    let source = open_regular(archive).map_err(io_err(archive))?;
    let bytes = read_capped(source, cap)?;
    verify_reader(&bytes[..], sha256_hex)?;
    let dir = beside(target, &format!("{STAGE_SUFFIX}-{}", std::process::id()));
    // `create_dir`, not `create_dir_all`: a folder already there isn't this run's, so it is neither used nor removed.
    fs::create_dir(&dir).map_err(io_err(&dir))?;
    let result = stage_from(io::Cursor::new(bytes), kind, &dir.join("staged"), &install)
        .and_then(|staged| apply(&staged, target, style));
    let _ = remove_any(&dir);
    result
}

/// All of `file`, refused without being read when it is (or grows) larger than `cap` bytes.
fn read_capped(file: File, cap: u64) -> Result<Vec<u8>, UpdateError> {
    let len = file.metadata().map_err(UpdateError::Read)?.len();
    if len > cap {
        return Err(UpdateError::TooLarge { limit: cap });
    }
    let mut bytes = Vec::with_capacity(len as usize);
    file.take(cap + 1).read_to_end(&mut bytes).map_err(UpdateError::Read)?;
    if bytes.len() as u64 > cap {
        return Err(UpdateError::TooLarge { limit: cap });
    }
    Ok(bytes)
}

/// macOS's `admin` group, which owns `/Applications` (root:admin 0775).
pub const MAC_ADMIN_GID: u32 = 80;

/// A folder's owner and mode (`st_mode`, file type included), as `lstat` reports them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FolderOwner {
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
}

const S_IFMT: u32 = 0o170000;
const S_IFDIR: u32 = 0o040000;

/// Whether a folder on the way to the target is out of other users' reach, for a helper running as `helper_uid`:
/// a real folder (not a link) owned by root or by the helper itself, writable by no group or others. On macOS a
/// folder of the `admin` group may be group-writable (`/Applications`): its members can get admin rights anyway.
///
/// Elevated, `helper_uid` is 0, so only root's folders pass. Otherwise a user who could change a folder could race
/// the helper's check-then-chmod of the new binary into a root `chmod` of any file.
pub fn trusted_folder(folder: FolderOwner, helper_uid: u32, os: Os) -> bool {
    let group_ok = folder.mode & 0o020 == 0 || (os == Os::Mac && folder.gid == MAC_ADMIN_GID);
    folder.mode & S_IFMT == S_IFDIR
        && (folder.uid == 0 || folder.uid == helper_uid)
        && folder.mode & 0o002 == 0
        && group_ok
}

/// Checks [`trusted_folder`] for the target's folder and every folder above it, up to `/`, for this process; the
/// first one that fails is the error. The target must be absolute; the updater passes a canonical path, so a link
/// on the way fails too.
#[cfg(unix)]
pub fn trusted_location(target: &Path) -> Result<(), PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let parent = target.parent().filter(|p| p.is_absolute()).ok_or_else(|| target.to_path_buf())?;
    // SAFETY: `geteuid` has no preconditions and can't fail.
    let helper_uid = unsafe { libc::geteuid() };
    for dir in parent.ancestors() {
        let meta = fs::symlink_metadata(dir).map_err(|_| dir.to_path_buf())?;
        let folder = FolderOwner { uid: meta.uid(), gid: meta.gid(), mode: meta.mode() };
        if !trusted_folder(folder, helper_uid, Os::current()) {
            return Err(dir.to_path_buf());
        }
    }
    Ok(())
}

/// Opens `path` for reading only if it is a regular file itself: a link is refused, not followed, and a FIFO or a
/// device fails at once instead of blocking. The check is made on the opened handle, so nothing can be swapped in
/// after it.
#[cfg(unix)]
fn open_regular(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("not a regular file"));
    }
    Ok(file)
}

/// Opens `path` for reading only if it is a regular file itself. A symlink or other reparse point is opened as itself
/// (`FILE_FLAG_OPEN_REPARSE_POINT`), not followed, and refused; the check is made on the opened handle.
#[cfg(windows)]
fn open_regular(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT};
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(io::Error::other("a link, not a regular file"));
    }
    let file = fs::OpenOptions::new().read(true).custom_flags(FILE_FLAG_OPEN_REPARSE_POINT).open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() || meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::other("not a regular file"));
    }
    Ok(file)
}

#[cfg(not(any(unix, windows)))]
fn open_regular(path: &Path) -> io::Result<File> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("not a regular file"));
    }
    Ok(file)
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

/// Copies a file or a whole tree into newly created files and folders ([`move_into`] across volumes), with the
/// permissions kept minus any set-id or sticky bits. Links are recreated on Unix, never followed; elsewhere a link
/// fails the copy, as does anything that isn't a file, a folder or a link.
pub fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(src)?;
    if meta.is_dir() {
        fs::create_dir(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            copy_tree(&entry.path(), &dst.join(entry.file_name()))?;
        }
        fs::set_permissions(dst, plain_permissions(&meta))
    } else if meta.is_symlink() {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(fs::read_link(src)?, dst)
        }
        #[cfg(not(unix))]
        {
            Err(io::Error::other(format!("{} is a link", src.display())))
        }
    } else if meta.is_file() {
        let mut from = File::open(src)?;
        let mut to = fs::OpenOptions::new().write(true).create_new(true).open(dst)?;
        io::copy(&mut from, &mut to)?;
        drop(to);
        fs::set_permissions(dst, plain_permissions(&meta))
    } else {
        Err(io::Error::other(format!("{} is not a file, a folder or a link", src.display())))
    }
}

/// `meta`'s permissions without set-user-id, set-group-id or sticky bits.
fn plain_permissions(meta: &fs::Metadata) -> fs::Permissions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::Permissions::from_mode(meta.permissions().mode() & 0o777)
    }
    #[cfg(not(unix))]
    {
        meta.permissions()
    }
}

/// A build folder under the work dir, or an elevated helper's stage folder beside the target, with anything modified
/// this recently belongs to an update in progress, maybe in another rogg process (there is no single-instance lock):
/// `sweep` leaves it, and the names beside the target, alone.
pub const SWEEP_IN_USE: Duration = Duration::from_secs(10 * 60);

/// Removes what an update leaves behind (best effort): stale build folders in `work_dir` (`<cache>/update`), and, when
/// no update is in progress, the names beside the target and the work dir itself.
pub fn sweep(target: Option<&Path>, work_dir: &Path) {
    sweep_at(target, work_dir, SystemTime::now());
}

/// `sweep` as of `now`.
pub fn sweep_at(target: Option<&Path>, work_dir: &Path, now: SystemTime) {
    // A modification time in the future (a changed clock) counts as fresh. Freshness relies on what the job writes
    // itself: the download (`.part`, then the archive) and the folders it creates. Unpacked files may keep the
    // archive's old times, so staging must never backdate the folders it makes.
    let fresh =
        |path: &Path| newest_change(path).is_some_and(|t| now.duration_since(t).map_or(true, |age| age < SWEEP_IN_USE));
    let mut in_use = false;
    for entry in fs::read_dir(work_dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if fresh(&path) {
            in_use = true;
        } else {
            let _ = remove_any(&path);
        }
    }
    // An elevated helper works beside the target, not in the work dir.
    let stages = target.map(stage_dirs).unwrap_or_default();
    if in_use || stages.iter().any(|dir| fresh(dir)) {
        return;
    }
    if let Some(target) = target {
        for dir in &stages {
            let _ = remove_any(dir);
        }
        // With the target gone, an old copy is the only copy of the program: restore it rather than delete it.
        let _ = restore_old(target);
        for suffix in [NEW_SUFFIX, WINDOWS_OLD_SUFFIX] {
            let _ = remove_any(&beside(target, suffix));
        }
        // If the restore failed, the old copies are all that is left: keep them.
        if fs::symlink_metadata(target).is_ok() {
            for old in std::iter::once(beside(target, OLD_SUFFIX)).chain(unique_olds(target)) {
                let _ = remove_any(&old);
            }
        }
    }
    let _ = remove_any(work_dir);
}

/// The latest modification time of `path` or anything under it (symlinks not followed).
fn newest_change(path: &Path) -> Option<SystemTime> {
    let meta = fs::symlink_metadata(path).ok()?;
    let mut newest = meta.modified().ok();
    if meta.is_dir() {
        for entry in fs::read_dir(path).into_iter().flatten().flatten() {
            newest = newest.max(newest_change(&entry.path()));
        }
    }
    newest
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
