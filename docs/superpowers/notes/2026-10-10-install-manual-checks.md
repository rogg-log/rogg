# install.sh — manual checks (once the repository is public)

- macOS, fresh: `curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash`.
  Choose rogg.app → /Applications: no sudo for an admin user (/Applications is admin-writable), sudo for a standard user; rogg.app opens from Finder with its icon, no
  Gatekeeper prompt.
- macOS, binary: choose 2 → ~/.local/bin; the PATH offer edits ~/.zshrc; a new terminal runs `rogg --version`.
- macOS, upgrade: after a newer build is published, run again; it shows "Installed: … build N" and "Installing:
  … build N+1".
- Linux (container or a desktop): binary install, the PATH offer, the applications-menu entry with the icon, and
  rogg starting from the menu.
- `--uninstall` on both: lists what it found with versions, removes it, offers to remove the PATH line, and leaves
  settings and data.
- A run with no terminal (`echo | bash install.sh` inside `ssh -T`): stops with the terminal message.

## Windows (install.ps1), on Windows 10/11 with Windows PowerShell 5.1 and PowerShell 7

- Fresh: `irm https://raw.githubusercontent.com/rogg-log/rogg/main/install.ps1 | iex` in Windows PowerShell 5.1 →
  default folder `%LOCALAPPDATA%\Programs\rogg`; the PATH offer; the Start menu offer. A new terminal runs rogg; Start
  shows rogg with its icon; Explorer shows the icon on rogg.exe, and its Properties → Details show product version
  `rogg 0.1.0 (build N, main, <sha>)`.
- After the run, the PowerShell window is still open, `$ErrorActionPreference` is unchanged, and `Get-Command Install-Rogg`
  finds nothing (nothing leaked into the session).
- Upgrade with rogg open: run again after a newer build → "Installed: … build N" / "Installing: … build N+1"; it
  succeeds while rogg runs; the next run removes the leftover `rogg.exe.*.old`.
- PATH with `%VAR%` entries (e.g. add `%USERPROFILE%\bin` first): after the install, the entry is still unexpanded in
  System → Environment Variables.
- A checksum mismatch (point `$env:ROGG_INSTALL_BASE` at a copy with a broken SHA256SUMS): an error, nothing changed.
- `-Uninstall`: lists the install with its version; removes the files, the Start menu entry, and (after asking) the
  PATH entry; keeps `%APPDATA%\rogg`.
