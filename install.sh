#!/usr/bin/env bash
# shellcheck shell=bash
# rogg installer for macOS and Linux (docs/superpowers/specs/2026-10-10-rogg-install-script-design.md):
#   curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash -s -- --uninstall
# It asks before changing anything, checks downloads against SHA256SUMS, and never touches settings or data.
# Answers are read from the terminal (when piped, stdin is this script). Test seams, not user options:
# ROGG_INSTALL_BASE (release folder URL), ROGG_INSTALL_INPUT (answers file), ROGG_INSTALL_PLATFORM.
# Everything is a function and main runs on the last line, so a cut-off download runs nothing.
set -euo pipefail

BASE=${ROGG_INSTALL_BASE:-https://github.com/rogg-log/rogg/releases/latest/download}
MARK="# added by rogg install.sh"

say() { printf '%s\n' "$*"; }
die() { printf 'rogg install: %s\n' "$*" >&2; exit 1; }

# ask <prompt> <default>: prints the answer (the default for an empty line or no more input).
ask() {
  local reply=""
  printf '%s ' "$1" >&2
  IFS= read -r reply <&3 || true
  [[ -t 3 ]] || printf '\n' >&2
  printf '%s' "${reply:-$2}"
}

# yes_no <question> <y|n>: succeeds for yes.
yes_no() {
  local hint="[y/N]" reply
  [[ $2 == y ]] && hint="[Y/n]"
  reply=$(ask "$1 $hint" "$2")
  case $reply in
    [Yy]*) return 0 ;;
    [Nn]*) return 1 ;;
    *) [[ $2 == y ]] ;;
  esac
}

platform() {
  local p=${ROGG_INSTALL_PLATFORM:-}
  if [[ -z $p ]]; then
    case "$(uname -s)/$(uname -m)" in
      Darwin/*) p=macos-universal ;;
      Linux/x86_64 | Linux/amd64) p=linux-x86_64 ;;
      Linux/aarch64 | Linux/arm64) p=linux-arm64 ;;
      *) p="$(uname -s)-$(uname -m)" ;;
    esac
  fi
  case $p in
    macos-universal | linux-x86_64 | linux-arm64) printf '%s' "$p" ;;
    *) die "rogg has no build for $p; builds exist for macOS (Apple silicon and Intel) and Linux (x86_64, arm64)" ;;
  esac
}

fetch() { curl -fsSL --retry 2 -o "$2" "$BASE/$1" || die "could not download $1 from $BASE"; }

# field <name> <manifest>: a top-level manifest.json value (strings unquoted).
field() { sed -n "s/^  \"$1\": \"\{0,1\}\([^\",]*\)\"\{0,1\},\{0,1\}\$/\1/p" "$2" | head -1; }

sha256() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi; }

# installed_version <binary>: "0.1.0 5", "0.1.0 local", "unknown" (no --version), or nothing when absent.
installed_version() {
  local out ci_re='^rogg ([0-9]+\.[0-9]+\.[0-9]+) \(build ([0-9]+)' local_re='^rogg ([0-9]+\.[0-9]+\.[0-9]+) \(local'
  [[ -x $1 ]] || return 0
  out=$("$1" --version 2>/dev/null </dev/null) || { printf unknown; return 0; }
  if [[ $out =~ $ci_re ]]; then printf '%s %s' "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}"
  elif [[ $out =~ $local_re ]]; then printf '%s local' "${BASH_REMATCH[1]}"
  else printf unknown; fi
}

describe() {
  case $1 in
    "") printf 'not installed' ;;
    unknown) printf 'rogg, unknown version' ;;
    *" local") printf 'rogg %s, a local build' "${1% *}" ;;
    *) printf 'rogg %s build %s' "${1% *}" "${1#* }" ;;
  esac
}

# writable <dir>: the folder, or its nearest existing parent, can be written without sudo.
writable() { local d=$1; while [[ ! -d $d ]]; do d=$(dirname "$d"); done; [[ -w $d ]]; }

expand_path() {
  local p=$1
  [[ $p == "~" ]] && p=$HOME
  # shellcheck disable=SC2088 # matches a typed, literal ~/ prefix to expand it
  [[ $p == "~/"* ]] && p=$HOME/${p#"~/"}
  [[ -n $p ]] || die "no folder given"
  printf '%s' "$p"
}

# pretty <path>: the path with the home folder shown as ~, for messages.
# shellcheck disable=SC2088 # a literal ~ is the point: it is shown, never expanded.
pretty() { if [[ $1 == "$HOME"/* ]]; then printf '~/%s' "${1#"$HOME"/}"; else printf '%s' "$1"; fi; }

on_path() { case ":$PATH:" in *":$1:"*) return 0 ;; *) return 1 ;; esac; }

shell_rc() {
  case $(basename "${SHELL:-sh}") in
    zsh) printf '%s' "${ZDOTDIR:-$HOME}/.zshrc" ;;
    bash) if [[ $PLATFORM == macos-* ]]; then printf '%s' "$HOME/.bash_profile"; else printf '%s' "$HOME/.bashrc"; fi ;;
    fish) printf '%s' "$HOME/.config/fish/conf.d/rogg.fish" ;;
    *) printf '%s' "$HOME/.profile" ;;
  esac
}

path_line() {
  local d=$1
  [[ $d == "$HOME"/* ]] && d="\$HOME/${d#"$HOME"/}"
  # shellcheck disable=SC2016 # $PATH is written literally into the startup file, expanded by the user's shell later
  if [[ $(basename "${SHELL:-sh}") == fish ]]; then printf 'set -gx PATH "%s" $PATH  %s' "$d" "$MARK"
  else printf 'export PATH="%s:$PATH"  %s' "$d" "$MARK"; fi
}

add_to_path() {
  local dir=$1 rc line
  on_path "$dir" && return 0
  rc=$(shell_rc); line=$(path_line "$dir")
  if [[ -f $rc ]] && grep -qF -- "$line" "$rc"; then
    say "$(pretty "$dir") is added to PATH in $(pretty "$rc"); open a new terminal to use rogg."
    return 0
  fi
  if yes_no "$(pretty "$dir") is not on your PATH. Add it in $(pretty "$rc")?" y; then
    mkdir -p "$(dirname "$rc")"
    [[ -s $rc && -n $(tail -c 1 "$rc") ]] && printf '\n' >> "$rc"
    printf '%s\n' "$line" >> "$rc"
    say "Added to $(pretty "$rc"). Open a new terminal to use rogg."
  fi
}

desktop_entry() {
  local target=$1 icon=$2 data=${XDG_DATA_HOME:-$HOME/.local/share}
  yes_no "Add rogg to your applications menu?" y || return 0
  mkdir -p "$data/icons/hicolor/256x256/apps" "$data/applications"
  cp "$icon" "$data/icons/hicolor/256x256/apps/rogg.png"
  cat > "$data/applications/rogg.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=rogg
Comment=Log viewer
Exec="$target" %F
Icon=rogg
Terminal=false
Categories=Development;Utility;
MimeType=text/plain;text/x-log;
DESKTOP
  if command -v update-desktop-database >/dev/null 2>&1; then update-desktop-database "$data/applications" >/dev/null 2>&1 || true; fi
  say "Added rogg to the applications menu."
}

main_install() {
  local version build kind=binary dir target bin archive current have want got sudo="" tmp
  # Global, so the EXIT trap still sees it after this function returns.
  TMP_DIR=$(mktemp -d)
  trap 'rm -rf "$TMP_DIR"' EXIT
  tmp=$TMP_DIR
  fetch manifest.json "$tmp/manifest.json"
  version=$(field version "$tmp/manifest.json"); build=$(field build "$tmp/manifest.json")
  [[ -n $version && $build =~ ^[0-9]+$ ]] || die "the release manifest could not be read"
  say "rogg installer — latest release: rogg $version build $build"

  if [[ $PLATFORM == macos-* ]]; then
    say "What to install?"
    say "  1) rogg.app, opens from Finder (recommended)"
    say "  2) the command-line binary"
    case $(ask "Choose [1]:" 1) in 1) kind=app ;; 2) kind=binary ;; *) die "choose 1 or 2" ;; esac
  fi
  if [[ $kind == app ]]; then
    say "Where?"
    say "  1) /Applications (recommended)"
    say "  2) ~/Applications"
    say "  3) another folder"
    case $(ask "Choose [1]:" 1) in
      1) dir=/Applications ;;
      2) dir=$HOME/Applications ;;
      3) dir=$(expand_path "$(ask "Folder:" "")") ;;
      *) die "choose 1, 2 or 3" ;;
    esac
    target=$dir/rogg.app; bin=$target/Contents/MacOS/rogg; archive=rogg-macos-universal-app.tar.gz
  else
    dir=$(expand_path "$(ask "Install the binary to [~/.local/bin]:" "$HOME/.local/bin")")
    target=$dir/rogg; bin=$target; archive=rogg-$PLATFORM.tar.gz
  fi

  current=$(installed_version "$bin")
  say "Installed:  $(describe "$current") ($(pretty "$target"))"
  say "Installing: rogg $version build $build"
  local re='^[0-9.]+ ([0-9]+)$'
  if [[ $current =~ $re ]]; then
    have=${BASH_REMATCH[1]}
    if (( have == build )); then
      yes_no "Already up to date. Reinstall?" n || { say "Nothing changed."; return 0; }
    elif (( have > build )); then
      yes_no "The installed build is newer than the latest release. Downgrade?" n || { say "Nothing changed."; return 0; }
    else
      yes_no "Install?" y || { say "Nothing changed."; return 0; }
    fi
  else
    yes_no "Install?" y || { say "Nothing changed."; return 0; }
  fi

  fetch "$archive" "$tmp/$archive"
  fetch SHA256SUMS "$tmp/SHA256SUMS"
  want=$(awk -v f="$archive" '$2 == f { print $1 }' "$tmp/SHA256SUMS")
  [[ -n $want ]] || die "$archive is not listed in SHA256SUMS; nothing was changed"
  got=$(sha256 "$tmp/$archive")
  [[ $got == "$want" ]] || die "checksum mismatch for $archive; nothing was changed"
  mkdir -p "$tmp/x"
  tar -xzf "$tmp/$archive" -C "$tmp/x"

  if ! writable "$dir"; then
    command -v sudo >/dev/null 2>&1 || die "$dir is not writable and sudo is not available"
    say "$(pretty "$dir") needs administrator rights; sudo will ask for your password."
    sudo=sudo
  fi
  $sudo mkdir -p "$dir"
  if [[ $kind == app ]]; then
    $sudo rm -rf "$target.old"
    [[ -e $target ]] && $sudo mv "$target" "$target.old"
    if ! $sudo mv "$tmp/x/rogg.app" "$target"; then
      [[ -e $target.old ]] && $sudo mv "$target.old" "$target"
      die "could not install $target; the previous version is back in place"
    fi
    $sudo rm -rf "$target.old"
    say "Installed rogg $version build $build to $(pretty "$target"). Open it from Finder, or: open \"$target\""
  else
    $sudo cp "$tmp/x/rogg" "$dir/.rogg.new.$$"
    $sudo chmod 755 "$dir/.rogg.new.$$"
    $sudo mv -f "$dir/.rogg.new.$$" "$target"
    say "Installed rogg $version build $build to $(pretty "$target")."
    add_to_path "$dir"
    if [[ $PLATFORM == linux-* ]]; then desktop_entry "$target" "$tmp/x/rogg.png"; fi
  fi
}

main_uninstall() { die "--uninstall is not implemented yet"; }

main() {
  local input=${ROGG_INSTALL_INPUT:-/dev/tty}
  (: <"$input") 2>/dev/null || die "no terminal to ask questions on; run the installer from a terminal"
  exec 3<"$input"
  PLATFORM=$(platform)
  case ${1:-} in
    "") main_install ;;
    --uninstall) main_uninstall ;;
    -h | --help) say "usage: install.sh [--uninstall]" ;;
    *) die "unknown option $1 (usage: install.sh [--uninstall])" ;;
  esac
}

main "$@"
