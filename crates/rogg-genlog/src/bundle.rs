//! Archives of synthetic logs (spec §3.1 bundle generator) and the archive fixtures of the rogg tests (spec §8: zip,
//! tar, tar.gz, tgz, gz, nested, corrupt).

use crate::{GenOptions, GenStats, generate_file};
use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleKind {
    Zip,
    Tar,
    TarGz,
}

impl BundleKind {
    pub fn parse(s: &str) -> Option<BundleKind> {
        match s {
            "zip" => Some(BundleKind::Zip),
            "tar" => Some(BundleKind::Tar),
            "tgz" | "tar.gz" => Some(BundleKind::TarGz),
            _ => None,
        }
    }
}

fn zip_options(method: zip::CompressionMethod) -> zip::write::SimpleFileOptions {
    zip::write::SimpleFileOptions::default().compression_method(method).large_file(true)
}

fn tar_append<W: Write>(builder: &mut tar::Builder<W>, name: &str, len: u64, data: impl Read) -> io::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(len);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, name, data)
}

/// Writes `members` (path, bytes) into a zip (deflated), a tar or a tar.gz at `path`.
pub fn write_archive(path: &Path, kind: BundleKind, members: &[(&str, &[u8])]) -> io::Result<()> {
    let file = File::create(path)?;
    match kind {
        BundleKind::Zip => write_zip(file, members, zip::CompressionMethod::Deflated),
        BundleKind::Tar => {
            let mut builder = tar::Builder::new(file);
            for (name, bytes) in members {
                tar_append(&mut builder, name, bytes.len() as u64, *bytes)?;
            }
            builder.into_inner()?.flush()
        }
        BundleKind::TarGz => {
            let mut builder = tar::Builder::new(GzEncoder::new(file, Compression::fast()));
            for (name, bytes) in members {
                tar_append(&mut builder, name, bytes.len() as u64, *bytes)?;
            }
            builder.into_inner()?.finish()?.flush()
        }
    }
}

fn write_zip(file: File, members: &[(&str, &[u8])], method: zip::CompressionMethod) -> io::Result<()> {
    let mut zip = zip::ZipWriter::new(file);
    for (name, bytes) in members {
        zip.start_file(*name, zip_options(method)).map_err(io::Error::other)?;
        zip.write_all(bytes)?;
    }
    zip.finish().map_err(io::Error::other)?.flush()
}

/// A zip whose members are stored uncompressed, so `damage_zip_member` can corrupt their data in place.
pub fn write_stored_zip(path: &Path, members: &[(&str, &[u8])]) -> io::Result<()> {
    write_zip(File::create(path)?, members, zip::CompressionMethod::Stored)
}

/// Flips one byte in the middle of member `index`'s data (its CRC no longer matches; the directory stays intact).
pub fn damage_zip_member(path: &Path, index: usize) -> io::Result<()> {
    let (header, size) = {
        let mut zip = zip::ZipArchive::new(File::open(path)?).map_err(io::Error::other)?;
        let entry = zip.by_index_raw(index).map_err(io::Error::other)?;
        (entry.header_start(), entry.compressed_size())
    };
    let mut file = File::options().read(true).write(true).open(path)?;
    // Local file header: 30 fixed bytes, then the name and the extra field, then the member's data.
    let mut fixed = [0u8; 30];
    file.seek(SeekFrom::Start(header))?;
    file.read_exact(&mut fixed)?;
    let name_len = u16::from_le_bytes([fixed[26], fixed[27]]) as u64;
    let extra_len = u16::from_le_bytes([fixed[28], fixed[29]]) as u64;
    let at = header + 30 + name_len + extra_len + size / 2;
    let mut byte = [0u8];
    file.seek(SeekFrom::Start(at))?;
    file.read_exact(&mut byte)?;
    file.seek(SeekFrom::Start(at))?;
    file.write_all(&[byte[0] ^ 0xff])
}

pub fn gz_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(bytes).expect("writing to memory");
    encoder.finish().expect("writing to memory")
}

pub fn write_gz(path: &Path, bytes: &[u8]) -> io::Result<()> {
    std::fs::write(path, gz_bytes(bytes))
}

/// `files` synthetic logs `service-NN.log` of `opts.size_bytes` each (seeds `opts.seed + i`) archived into `out`.
/// Each log is generated into a temporary file next to `out` and streamed into the archive, so bundles of 1 GB logs
/// never sit in memory.
pub fn generate_bundle(out: &Path, kind: BundleKind, files: usize, opts: &GenOptions) -> io::Result<GenStats> {
    let dir = out.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let names: Vec<String> = (0..files).map(|i| format!("service-{i:02}.log")).collect();
    let temps: Vec<PathBuf> = names.iter().map(|n| dir.join(format!(".genlog-{}-{n}", std::process::id()))).collect();
    let mut stats = GenStats::default();
    let result = (|| -> io::Result<()> {
        for (i, temp) in temps.iter().enumerate() {
            let s = generate_file(temp, &GenOptions { seed: opts.seed.wrapping_add(i as u64), ..opts.clone() })?;
            stats.lines += s.lines;
            stats.bytes += s.bytes;
        }
        let file = File::create(out)?;
        match kind {
            BundleKind::Zip => {
                let mut zip = zip::ZipWriter::new(file);
                for (name, temp) in names.iter().zip(&temps) {
                    zip.start_file(name.as_str(), zip_options(zip::CompressionMethod::Deflated)).map_err(io::Error::other)?;
                    io::copy(&mut File::open(temp)?, &mut zip)?;
                }
                zip.finish().map_err(io::Error::other)?.flush()
            }
            BundleKind::Tar => tar_files(file, &names, &temps)?.flush(),
            BundleKind::TarGz => tar_files(GzEncoder::new(file, Compression::fast()), &names, &temps)?.finish()?.flush(),
        }
    })();
    for temp in &temps {
        let _ = std::fs::remove_file(temp);
    }
    result.map(|()| stats)
}

/// Streams each file of `temps` into a tar under the matching name; returns the finished tar's sink.
fn tar_files<W: Write>(sink: W, names: &[String], temps: &[PathBuf]) -> io::Result<W> {
    let mut builder = tar::Builder::new(sink);
    for (name, temp) in names.iter().zip(temps) {
        let source = File::open(temp)?;
        let len = source.metadata()?.len();
        tar_append(&mut builder, name, len, source)?;
    }
    builder.into_inner()
}
