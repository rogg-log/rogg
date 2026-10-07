# M3 hand checks

The automated tests cover the behaviour; these checks cover what only a real machine shows (FSEvents, real bundles,
memory, the native window). Use release builds (`just demo`, `just run …`).

## Bundles
- `just bundle tgz 10 1G target/genlog/big.tgz`, then `just run target/genlog/big.tgz`: the sidebar lists 10 members
  with sizes; the first member opens with `Extracting service-00.log` progress and a working Cancel; the footer reads
  `10 members · 10.7 GB expanded` (genlog's `1G` is 1 GiB; sizes are shown in decimal units) /
  `Members open individually`.
- A real support bundle (zip with nested `.tgz` and rotated `*.log.N.gz`): nested archives list on first expand
  (`listing…`), `.gz` members decompress, a damaged member shows the error state with Retry, other members open.
- Reopen a member: it opens at once (cache hit). `~/Library/Caches/rogg` holds `*.member` / `*.log` entries; with
  the limit lowered by hand in code to 1 GB, closed members are evicted on the next open; open ones never are.

## Sidebar
- Open a folder with ~10,000 files (`mkdir -p target/genlog/many && cd target/genlog/many && seq 1 10000 | xargs -I{} touch f{}.log`,
  then `just run target/genlog/many`): the scan stays bounded and the toast says when it stopped early. Known limit
  to observe (ruling R12): sidebar rows are not virtualized yet, so note how long the window takes to appear and
  whether scrolling the tree stays smooth.

## Compressed files
- `gzip -k` a 1 GB log and open the `.gz`: rows appear while the footer shows `Decompressing N%`; scrolling and
  filtering work meanwhile; the end arrives without a reload.

## Live follow
- `just live` in one terminal, `just run target/genlog/live.log` in another: new lines scroll in (`Pause` control);
  Space pauses (`Paused +N` counts), the wheel up pauses too; Space resumes at the end.
- `: > target/genlog/live.log` while it runs: the toast `File was truncated or replaced — reloaded.`, no crash.
- Rotation: `mv live.log live.log.1` then let `just live` recreate it: the tab reloads the new file.
- Truncate a 1 GB file (`: > big.log`) while scrolling it: no crash (SIGBUS guard), then the reload.

## stdin and FIFOs
- `just genlog` first (it writes `target/genlog/sample.log`), then `just pipe`: a `stdin` tab fills while `cat` runs
  and follows its end. Closing the tab with ⌘W stops the capture and deletes its file under `streams/`.
- Starting rogg from Finder (stdin is `/dev/null`) opens no stdin tab.
- A real piped launch: `printf 'one\ntwo\n' | just run` opens a `stdin` tab with two lines (a pipe on a real
  terminal, not the test harness's stand-in).
- `rogg <(tail -f target/genlog/live.log)` hands rogg `/dev/fd/N`: the path resolves to a stream tab (not a
  missing-file error) and the tab keeps following new lines.

## Window
- ⌘H hides rogg, ⌥⌘H hides others, the rogg menu has Show All.
- Dragging the filter band or the sidebar edge never jumps at the start, and the resize cursor stays during the drag.
