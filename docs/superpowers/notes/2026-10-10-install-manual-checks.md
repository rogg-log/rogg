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
