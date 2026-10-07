//! Exporting filtered lines to a file (spec §6.5): the shown lines, optionally without context lines, with gap markers
//! and original line numbers. Written to a writer-unique partial file and renamed, so a failure leaves nothing behind.

use crate::cache::partial_path;
use crate::cancel::CancelToken;
use crate::error::CoreError;
use crate::stats::noun;
use crate::text::LogText;
use crate::view::FilteredView;
use std::io::{BufWriter, Write};
use std::path::Path;

const BATCH: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportOptions {
    /// Include context lines (off: only matched and pinned lines).
    pub context: bool,
    /// A `··· N lines omitted` line where lines were skipped.
    pub gaps: bool,
    /// Prefix each line with its original display number.
    pub numbers: bool,
}

pub fn gap_marker(omitted: u64) -> String {
    format!("··· {omitted} {} omitted", noun(omitted, "line", "lines"))
}

/// Writes the view's lines of `text` to `dst`; returns the number of log lines written (gap markers not counted).
pub fn export_lines(
    text: &LogText,
    view: &FilteredView,
    number_of: impl Fn(u64) -> u64,
    options: ExportOptions,
    dst: &Path,
    cancel: &CancelToken,
    mut progress: impl FnMut(u64, u64),
) -> Result<u64, CoreError> {
    let shown = view.shown_lines();
    let keep = |line: u64| options.context || view.is_anchor(line);
    let width = if options.numbers { shown.iter().rev().find(|&&l| keep(l)).map_or(1, |&l| number_of(l).to_string().len()) } else { 0 };
    let partial = partial_path(dst);
    let result = (|| -> Result<u64, CoreError> {
        let file = std::fs::File::create(&partial).map_err(CoreError::io(dst))?;
        let mut out = BufWriter::with_capacity(1 << 20, file);
        let total = shown.len() as u64;
        let mut written = 0u64;
        let mut prev: Option<u64> = None;
        for (k, &line) in shown.iter().enumerate() {
            if k % BATCH == 0 {
                if cancel.is_cancelled() {
                    return Err(CoreError::Cancelled);
                }
                progress(k as u64, total);
            }
            if !keep(line) {
                continue;
            }
            let io = CoreError::io(dst);
            if options.gaps
                && let Some(p) = prev
                && line > p + 1
            {
                writeln!(out, "{}", gap_marker(line - p - 1)).map_err(io)?;
            }
            if options.numbers {
                write!(out, "{:>width$}  ", number_of(line)).map_err(CoreError::io(dst))?;
            }
            out.write_all(text.line(line)).and_then(|_| out.write_all(b"\n")).map_err(CoreError::io(dst))?;
            written += 1;
            prev = Some(line);
        }
        out.flush().map_err(CoreError::io(dst))?;
        progress(total, total);
        Ok(written)
    })();
    match result {
        Ok(n) => std::fs::rename(&partial, dst).map(|_| n).map_err(|e| {
            let _ = std::fs::remove_file(&partial);
            CoreError::Io { path: dst.to_path_buf(), source: e }
        }),
        Err(e) => {
            let _ = std::fs::remove_file(&partial);
            Err(e)
        }
    }
}
