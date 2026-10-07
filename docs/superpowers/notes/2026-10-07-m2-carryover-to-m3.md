# M2 → M3 carry-over

Known limits and follow-ups from the M2 final review, grouped by area. None blocks the M2 milestone.

## Engine / UI thread
- R10: the `FilteredView` rebuild on a pin or context change runs on the UI thread and is O(matches × context) (about 0.2 s at 1 M matches with ±3 context, release). Move large rebuilds off-thread, keep the current view until the new one lands, and drop stale results by revision.
- A filtered search restarts on every progressive publish. Skip the restart while `filter.progress` is `Some`.
- The search label can stick at `…` if a search worker dies. Show a failed state.
- Invalid UTF-8 can expand `display_line` up to about 3× and show a false `…` on lines of 64 KB or less.

## Sources
- FIFOs and pipes (`len 0`) and hung network mounts take the synchronous open path on the UI thread.
- Duplicate tabs across path spellings: a missing file opened raw, later retried, then opened canonical.

## Log view
- The full log pane rebuilds its rows on every progressive publish (a pins revision would avoid it; the rebuild also clears wrapped heights).
- An initial filtered selection is dropped when the first scan is async, and a selected line that is revealed later is not re-selected.
- Band drag jumps by up to 5 px (no grab offset), and the resize cursor drops mid-drag.
- The horizontal scrollbar hit zone overlaps the bottom row's lower ~11 px; decided together with the R3 golden re-approval.

## Chrome
- `rogg --demo FILE` leaves the file tab active.
- ⌘H (Hide) and the standard app menu items.

## Tests / tooling
- Pre-existing clippy warnings: rogg-ui `logview/element.rs` (large enum variants), `text.rs:299`, and the collapsible `if`s in gallery `replica/state.rs`.
