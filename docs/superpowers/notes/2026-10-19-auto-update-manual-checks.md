# Auto-update — manual checks (needs two published builds, N and N+1)

## The admin prompts, each approved and each cancelled

- [ ] macOS: `/Applications/rogg.app` as a standard user. Approving updates the app; cancelling the password prompt shows
  "Admin rights were declined." (osascript's `-128`) and changes nothing.
- [ ] Linux: `/usr/local/bin/rogg` via `pkexec`. Approving updates the binary; cancelling shows "Admin rights were declined."
- [ ] Windows: `C:\Program Files\rogg\rogg.exe` via UAC. Approving updates the exe; cancelling shows "Admin rights were declined."
- [ ] Windows: the UAC prompt appears in front of rogg (the launcher passes no parent window), and the update works
  without COM initialisation.
- [ ] Linux, on a desktop without a polkit agent: note what `pkexec` does and what the dialog says.
- [ ] After an elevated update the installed rogg is owned by root/admin, not the user (`ls -l` / `stat`, or
  Properties → Security on Windows).

## A real update from one published build to the next

After each, restart and check `rogg --version` (or the About dialog).

- [ ] `rogg.app` in `/Applications` as an admin user.
- [ ] The binary in `~/.local/bin` on macOS.
- [ ] The binary in `~/.local/bin` on Linux.
- [ ] `rogg.exe` in `%LOCALAPPDATA%\Programs\rogg`.

## Network

- [ ] Behaviour behind a proxy with its own root certificate.
- [ ] GitHub's `latest/download/manifest.json` and the release-tag asset URL both download through their 302
  redirects, on each OS.

## Concurrency

- [ ] Two rogg processes: start an update in one, launch a second rogg during the download; the first update still
  completes.
