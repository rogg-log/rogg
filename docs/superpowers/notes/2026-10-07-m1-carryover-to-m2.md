# M1 → M2 carry-over

Engine contracts M2 (main window wiring) must honor, and known limits deliberately deferred from M1. Fold them into the M2 plan.

## Filter jobs (`rogg_core::engine::FilterJob`)
- An event channel that closes without `Finished` or `Cancelled` means the worker died (panic). Treat it as an error, not a cancel.
- Never drop a `FilterJob` from a rayon pool thread. Drop cancels and joins the worker, which can deadlock there.
- After a cancel, stale `Chunk` events may arrive until the current wave ends. Discard them by job; one receiver per job makes this trivial.
- `wait()` after partially consuming `events()` returns a truncated or error result. Use one or the other.

## Display and highlighting
- Slice lines to 64 KB before `LogText::line_text` / `Highlighter::spans`. Both are O(line length) and allocate.

## Filtered view and stats
- `FilteredView::build`'s `excluded` closure is `|n| filter.is_excluded(text.line(n))`. `matches` must be sorted, unique and `< total_lines`.
- Decide whether to rescan by comparing terms, regex flag and case flag, not the whole `FilterSpec`. A context change only rebuilds the view.
- `FilterStats::compute` takes `matched` from the `FilterResult` and the rest from the `FilteredView`. Pass a matching pair.

## Persistence and copy
- History saves must be serialized: there is a single `.partial` name per file. (UTF-16 transcode partials are now unique per writer.)
- The stats summary pluralisation ("1 matches") is UI copy to fix in M2.

## Known deferred engine limits
- Peak heap during a 1 GB open is about 143 MB (steady state 41 MB).
- `run_filter` result peak is about 182 MB at 8.5 M matches.
- Cancelling inside a single huge line (hundreds of MB, no newline) takes 100+ ms.
- SIGBUS on external truncation of a mapped file (planned for M3).
