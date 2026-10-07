# M3 → M4 carry-over

Known limits and follow-ups from M3 (Sources) and its final review, grouped by area. None blocks the M3 milestone.
The plan's "Deferred from M3" list holds the same limits in short form.

## Engine / UI thread
- Each live-follow growth copies all matches (`TailBase::new` / `merge`) and the full-log search result on the UI thread, O(matches). The filtered-pane search recount also restarts on every growth.
- Each growth clones the whole line index on the follower thread, O(lines). Sharing the stable blocks would avoid it before spec-scale profiling.
- Superseded view builds are not cancelled: at most one background build runs per document and a stale result is dropped when it lands, but a running build is not interrupted. A dead view thread silently leaves the old view, and a search that ends in an error (not a death) still shows `…`.
- A dead tail scan's `term_hits` cover only the base (the partial-results error is shown). `stop_searches` is untested.
- A panic inside `open()` / a read in the stream or open worker leaves `done` false forever (no drop guard).

## Sources
- Opening a file on a hung network mount still stats it on the UI thread; a path swapped to a FIFO between `metadata` and `sniff` can block a scan; `classify()` sniffs on the UI thread.
- A plain file is indexed before it shows (about 150 ms per GB), so there is no progressive indexing.
- A UTF-16 file is not live-followed (its conversion is a snapshot); a file opened empty keeps UTF-8 / no BOM even if a BOM or UTF-16 is appended later.
- An in-place rewrite that keeps the file's first 64 bytes reads as an append.
- Listing a `.tar.gz` decompresses it once end to end; listing a plain `.tar` reads every byte (the `Seek` is hidden behind `Box<dyn Read>`), although the spec says "listed cheaply". Cancellation is only checked between tar entries, so skipping a huge member is uninterruptible.
- Trailing bytes after the last gzip member fail the decode after the data was shown. A truncated `.gz` switches the tab to the error state (spec §4.13) instead of keeping the decompressed part.
- Zip symlink entries are listed as files; output write errors are attributed to the archive path; `open_handle` transcodes UTF-16 by path, not through the handle.
- Two concurrent opens of one not-yet-decompressed `.gz` decompress twice (correct, duplicated work).
- A FIFO that fails to open shows an empty stream tab (the failure is a toast, not in the tab). `rogg -` on a terminal shows no "waiting for input" hint. A blocked read cannot be interrupted, so a stopped capture of an idle pipe lives until its next write; captures in `streams/` are not counted toward the cache limit and are not evicted (a finished stdin capture stays until it is swept).
- Cache: the lease-before-`exists()` contract is only implicit in the lease doc; the `process_alive` EPERM branch is untested (pid reuse is accepted); `CacheStore::get` falls back to a fresh `Cache` per call when no global is installed (all hosts install one); `transcoded_path` re-stats the path after the handle was read.
- `genlog`: `generate_bundle` peaks at files × size of temp disk and leaves a partial `out` on failure; the tar/tgz branches and `--bundle` / `--files` parsing are untested; `--files` is ignored without `--bundle`.

## Live follow
- Every live-followed tab keeps the 16 ms poll loop running for its lifetime (energy with many tabs); the follower thread spawn failing is silent.
- A never-laid-out background tab reports `at_bottom()` as true, so it follows on its first growth. The filtered pane leaving the tail keeps `live.following` true (the control still says `Pause`). A keyboard move to a non-last row leaves tail mode even if nothing scrolls (intended).
- Stream tabs are never restored by session restore (M4) and are not recent files.

## Sidebar
- Rows are not virtualized (R12): a large folder or archive level (e.g. 10,000 files or 50,000 flat members) builds every row. Virtualizing changes the shared `rogg-ui` `Sidebar` that the replica goldens capture, so it must keep the replica pixel-identical (or be re-approved with the user).
- Reopening an archive overwrites its remembered member in Recent; one archive that is both a folder child and a root maps to the first node; `insert_folder` / `insert_listing` are not idempotent (a re-list duplicates children); non-UTF-8 directory names that collapse to the same lossy name merge into one node; `summary()` walks the subtree on each call.
- `same_file` canonicalizes every open tab's path per open, on the UI thread.
- R3 resolved at the M2 gate (2026-10-07): the horizontal thumb shows only on hover over its strip, while dragged and briefly after a horizontal scroll, and is not clickable while hidden; the focus line of the panes and the filter band is 1 px on their perimeter. `just visual-check` passes again (0.000% on all 9 replica captures).

## Tests / tooling
- An app-level test that truncates a multi-page file mid-scan and renders is missing: Review Focus 1 is covered at the core level only (`truncation_under_a_mapping_reads_zeros_and_slots_are_released`) and by a small-file app test.
- SIGBUS guard: handler chaining to the previous action is one-way (the guard is removed after a foreign fault); slot exhaustion and the non-ours fallback are untested.
- Untested paths: unsupported zip method, truncated plain tar / zip member, duplicate tar paths, exact-limit folder truncation, hidden directories, mid-scan folder cancellation, non-UTF-8 names, follower end after `Truncated` / `Replaced`, delete-then-recreate as `Replaced`, the real sidebar `Streams` group, the `-` / `stdin_opened` guard, 3/4-byte UTF-8 straddles and a more-than-64 KB invalid line, the grab-offset wiring of the drag handles.
- The live e2e's final assertion does not discriminate a re-run filter (`fresh` has no ERROR). Several follower tests duplicate `wait_until`; test import order is uneven (`tests/tab.rs`).
- Drag-handle offsets duplicate literals in `filter.rs` / `layout.rs`; the `…` marker can be highlighted by a filter or search term; "Hide others" vs "Hide Others" casing.
- Hand checks that need a real machine are in `2026-10-07-m3-manual-checks.md`.
