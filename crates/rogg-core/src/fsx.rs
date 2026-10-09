//! Positional reads and file identity that behave the same on Unix and Windows.

use std::fs::File;
use std::io;
use std::path::Path;

/// Reads at `offset` without relying on the file's cursor (on Windows the cursor does move; rogg never reads these
/// handles sequentially).
pub fn read_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
    #[cfg(unix)]
    return std::os::unix::fs::FileExt::read_at(file, buf, offset);
    #[cfg(windows)]
    return std::os::windows::fs::FileExt::seek_read(file, buf, offset);
}

/// Fills `buf` from `offset`, failing with `UnexpectedEof` when the file is shorter.
pub fn read_exact_at(file: &File, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
    while !buf.is_empty() {
        match read_at(file, buf, offset) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "failed to fill whole buffer")),
            Ok(n) => {
                buf = &mut buf[n..];
                offset += n as u64;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Identifies the file behind an open handle: (device, inode) on Unix, (volume serial, file index) on Windows.
pub fn file_identity(file: &File) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = file.metadata().ok()?;
        Some((meta.dev(), meta.ino()))
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle};
        // SAFETY: the handle is valid for the borrow of `file`; the call only writes into `info`.
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut info) } == 0 {
            return None;
        }
        Some((u64::from(info.dwVolumeSerialNumber), (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow)))
    }
}

/// The identity of the file `path` names now; `None` when it cannot be examined.
pub fn path_identity(path: &Path) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::metadata(path).ok()?;
        Some((meta.dev(), meta.ino()))
    }
    #[cfg(windows)]
    {
        file_identity(&File::open(path).ok()?)
    }
}
