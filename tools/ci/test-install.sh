#!/usr/bin/env bash
# SC2016: fake scripts and expected PATH lines contain a literal $1/$HOME/$PATH on purpose.
# SC2015: `check && check || { fail ...; return; }` is the house style here; fail always runs the else branch.
# SC2088: some expected messages contain a literal ~/ (install.sh prints home paths that way).
# shellcheck disable=SC2016,SC2015,SC2088
# Tests for install.sh against a fake release served through file:// (a temporary HOME, scripted answers).
# Runs under macOS /bin/bash 3.2 and on Linux. Usage: bash tools/ci/test-install.sh
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
sh=${BASH:-bash}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
failures=0
pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s: %s\n' "$1" "$2"; failures=$((failures + 1)); }
version=$("$sh" "$root/tools/ci/version.sh")
sha256() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi; }

# A fake rogg whose --version reports <build>; with "old" it rejects --version like builds before it existed.
fake_rogg() {
  if [[ $2 == old ]]; then printf '#!/bin/sh\necho "unknown option" >&2\nexit 2\n' > "$1"
  else printf '#!/bin/sh\n[ "$1" = --version ] && echo "rogg %s (build %s, main, abc1234)"\nexit 0\n' "$version" "$2" > "$1"; fi
  chmod 755 "$1"
}

# A release folder for <build>: the three archives install.sh uses, SHA256SUMS and manifest.json.
release() {
  local build=$1 dir=$work/release-$1 s=$work/stage-$1
  [[ -d $dir ]] && { echo "$dir"; return; }
  mkdir -p "$dir" "$s/lin" "$s/bin" "$s/app/rogg.app/Contents/MacOS"
  fake_rogg "$s/lin/rogg" "$build"; cp "$root/crates/rogg-app/assets/icon/rogg.png" "$s/lin/"
  (cd "$s/lin" && tar -czf "$dir/rogg-linux-x86_64.tar.gz" rogg rogg.png)
  fake_rogg "$s/bin/rogg" "$build"
  (cd "$s/bin" && tar -czf "$dir/rogg-macos-universal.tar.gz" rogg)
  fake_rogg "$s/app/rogg.app/Contents/MacOS/rogg" "$build"; printf '<plist/>' > "$s/app/rogg.app/Contents/Info.plist"
  (cd "$s/app" && tar -czf "$dir/rogg-macos-universal-app.tar.gz" rogg.app)
  "$sh" "$root/tools/ci/manifest.sh" "$dir" "$build" abc1234 main
  echo "$dir"
}

# run <home> <platform> <release-dir> <answers, one per line> [-- install.sh args]; output in $work/out, status in $status
run() {
  local home=$1 plat=$2 rel=$3 answers=$4
  shift 4
  [[ ${1:-} == -- ]] && shift
  printf '%s' "$answers" > "$work/answers"
  set +e
  env -i HOME="$home" SHELL="${TEST_SHELL:-/bin/zsh}" PATH="${TEST_PATH:-/usr/bin:/bin}" TERM=dumb ${ZDOTDIR_TEST:+ZDOTDIR="$ZDOTDIR_TEST"} \
    ROGG_INSTALL_BASE="file://$rel" ROGG_INSTALL_INPUT="$work/answers" ROGG_INSTALL_PLATFORM="$plat" \
    "$sh" "$root/install.sh" "$@" > "$work/out" 2>&1
  status=$?
  set -e
}
new_home() { local h=$work/home-$1; rm -rf "$h"; mkdir -p "$h"; echo "$h"; }
has() { grep -qF -- "$1" "$work/out"; }

test_main_is_called_last() {
  local name=${FUNCNAME[0]}
  local last
  last=$(grep -v '^[[:space:]]*\(#.*\)\{0,1\}$' "$root/install.sh" 2>/dev/null | tail -1 || true)
  [[ $last == 'main "$@"' ]] || { fail "$name" "the last line is not main \"\$@\" (got '$last')"; return; }
  pass "$name"
}

test_fresh_binary_install_adds_path_once() {
  local name=${FUNCNAME[0]} h rel
  h=$(new_home fresh); rel=$(release 7)
  run "$h" linux-x86_64 "$rel" $'\n\n\nn\n'
  [[ $status == 0 ]] || { fail "$name" "exit $status: $(tail -3 "$work/out")"; return; }
  has "Installed:  not installed" && has "Installing: rogg $version build 7" || { fail "$name" "summary"; return; }
  [[ $("$h/.local/bin/rogg" --version) == *"build 7,"* ]] || { fail "$name" "binary not installed"; return; }
  [[ $(grep -c "added by rogg install.sh" "$h/.zshrc") == 1 ]] || { fail "$name" "PATH line not added once"; return; }
  grep -qF 'export PATH="$HOME/.local/bin:$PATH"  # added by rogg install.sh' "$h/.zshrc" || { fail "$name" "PATH line wording"; return; }
  run "$h" linux-x86_64 "$rel" $'\n\n'
  has "Already up to date" && has "Nothing changed" || { fail "$name" "second run did not say up to date"; return; }
  [[ $(grep -c "added by rogg install.sh" "$h/.zshrc") == 1 ]] || { fail "$name" "PATH line added twice"; return; }
  pass "$name"
}

test_upgrade_shows_both_builds() {
  local name=${FUNCNAME[0]} h
  h=$(new_home upgrade)
  run "$h" linux-x86_64 "$(release 6)" $'\n\nn\nn\n'
  run "$h" linux-x86_64 "$(release 7)" $'\n\nn\nn\n'
  has "Installed:  rogg $version build 6" && has "Installing: rogg $version build 7" || { fail "$name" "summary"; return; }
  [[ $("$h/.local/bin/rogg" --version) == *"build 7,"* ]] || { fail "$name" "not upgraded"; return; }
  pass "$name"
}

test_downgrade_is_refused_by_default() {
  local name=${FUNCNAME[0]} h
  h=$(new_home downgrade)
  run "$h" linux-x86_64 "$(release 7)" $'\n\nn\nn\n'
  run "$h" linux-x86_64 "$(release 6)" $'\n\n'
  has "newer than the latest release" && has "Nothing changed" || { fail "$name" "no downgrade warning"; return; }
  [[ $("$h/.local/bin/rogg" --version) == *"build 7,"* ]] || { fail "$name" "downgraded"; return; }
  pass "$name"
}

test_checksum_mismatch_changes_nothing() {
  local name=${FUNCNAME[0]} h bad
  h=$(new_home checksum)
  run "$h" linux-x86_64 "$(release 7)" $'\n\nn\nn\n'
  bad=$work/release-bad; rm -rf "$bad"; cp -R "$(release 8)" "$bad"
  sed -i.bak "s/^[0-9a-f]\{64\}  rogg-linux-x86_64.tar.gz/$(printf '0%.0s' $(seq 64))  rogg-linux-x86_64.tar.gz/" "$bad/SHA256SUMS"
  run "$h" linux-x86_64 "$bad" $'\n\n'
  [[ $status != 0 ]] && has "checksum mismatch" || { fail "$name" "exit $status without a mismatch error"; return; }
  [[ $("$h/.local/bin/rogg" --version) == *"build 7,"* ]] || { fail "$name" "the old binary was touched"; return; }
  pass "$name"
}

test_mac_app_installs_to_a_folder_with_spaces() {
  local name=${FUNCNAME[0]} h
  h=$(new_home macapp)
  run "$h" macos-universal "$(release 7)" "1"$'\n'"3"$'\n'"$h/My Apps"$'\n\n'
  [[ $status == 0 ]] || { fail "$name" "exit $status: $(tail -3 "$work/out")"; return; }
  [[ $("$h/My Apps/rogg.app/Contents/MacOS/rogg" --version) == *"build 7,"* ]] || { fail "$name" "app not installed"; return; }
  [[ ! -e $h/.zshrc ]] || { fail "$name" "touched PATH for an app install"; return; }
  run "$h" macos-universal "$(release 8)" "1"$'\n'"3"$'\n'"$h/My Apps"$'\n\n'
  has "Installed:  rogg $version build 7" || { fail "$name" "upgrade summary"; return; }
  [[ $("$h/My Apps/rogg.app/Contents/MacOS/rogg" --version) == *"build 8,"* && ! -e "$h/My Apps/rogg.app.old" ]] || { fail "$name" "app not replaced cleanly"; return; }
  pass "$name"
}

test_mac_binary_uses_bash_profile_for_bash() {
  local name=${FUNCNAME[0]} h
  h=$(new_home macbin)
  TEST_SHELL=/bin/bash run "$h" macos-universal "$(release 7)" $'2\n\n\n\n'
  [[ -x $h/.local/bin/rogg ]] || { fail "$name" "binary not installed"; return; }
  grep -q "added by rogg install.sh" "$h/.bash_profile" || { fail "$name" "no line in .bash_profile"; return; }
  pass "$name"
}

test_an_old_binary_without_version_shows_unknown_and_upgrades() {
  local name=${FUNCNAME[0]} h
  h=$(new_home old); mkdir -p "$h/.local/bin"; fake_rogg "$h/.local/bin/rogg" old
  run "$h" linux-x86_64 "$(release 7)" $'\n\nn\nn\n'
  has "unknown version" || { fail "$name" "did not say unknown version"; return; }
  [[ $("$h/.local/bin/rogg" --version) == *"build 7,"* ]] || { fail "$name" "not upgraded"; return; }
  pass "$name"
}

test_unsupported_platform_stops_cleanly() {
  local name=${FUNCNAME[0]} h
  h=$(new_home unsupported)
  run "$h" freebsd-x86_64 "$(release 7)" ""
  [[ $status != 0 ]] && has "no build for" || { fail "$name" "exit $status: $(cat "$work/out")"; return; }
  pass "$name"
}

test_no_terminal_stops_with_a_message() {
  local name=${FUNCNAME[0]} h
  h=$(new_home notty)
  set +e
  env -i HOME="$h" PATH=/usr/bin:/bin ROGG_INSTALL_BASE="file://$(release 7)" ROGG_INSTALL_INPUT=/nonexistent/tty \
    ROGG_INSTALL_PLATFORM=linux-x86_64 "$sh" "$root/install.sh" > "$work/out" 2>&1
  status=$?; set -e
  [[ $status != 0 ]] && has "terminal" || { fail "$name" "exit $status: $(cat "$work/out")"; return; }
  pass "$name"
}

test_linux_desktop_entry_and_uninstall() {
  local name=${FUNCNAME[0]} h data
  h=$(new_home uninstall); data=$h/.local/share
  run "$h" linux-x86_64 "$(release 7)" $'\n\n\n\n'
  grep -qx "Exec=\"$h/.local/bin/rogg\" %F" "$data/applications/rogg.desktop" || { fail "$name" "desktop Exec"; return; }
  [[ -s $data/icons/hicolor/256x256/apps/rogg.png ]] || { fail "$name" "icon not installed"; return; }
  run "$h" linux-x86_64 "$(release 7)" $'\n\n' -- --uninstall
  [[ $status == 0 ]] || { fail "$name" "exit $status: $(tail -3 "$work/out")"; return; }
  has "rogg $version build 7" || { fail "$name" "did not list the install with its version"; return; }
  [[ ! -e $h/.local/bin/rogg && ! -e $data/applications/rogg.desktop && ! -e $data/icons/hicolor/256x256/apps/rogg.png ]] || { fail "$name" "left files behind"; return; }
  ! grep -q "added by rogg install.sh" "$h/.zshrc" || { fail "$name" "PATH line kept"; return; }
  has "Settings and data are kept" || { fail "$name" "did not mention settings and data"; return; }
  pass "$name"
}

test_path_line_lands_on_its_own_line() {
  local name=${FUNCNAME[0]} h
  h=$(new_home rcnewline); printf 'alias ll="ls -l"' > "$h/.zshrc"
  run "$h" linux-x86_64 "$(release 7)" $'\n\n\nn\n'
  [[ $(head -1 "$h/.zshrc") == 'alias ll="ls -l"' ]] || { fail "$name" "the last line was changed: $(head -1 "$h/.zshrc")"; return; }
  grep -qx 'export PATH="$HOME/.local/bin:$PATH"  # added by rogg install.sh' "$h/.zshrc" || { fail "$name" "PATH line not on its own line"; return; }
  pass "$name"
}

test_uninstall_keeps_the_rest_of_a_startup_file() {
  local name=${FUNCNAME[0]} h zd
  h=$(new_home rckeep); zd=$h/zdot; mkdir -p "$zd"
  printf 'alias ll="ls -l"\nexport EDITOR=vim\n' > "$zd/.zshrc"
  ZDOTDIR_TEST=$zd run "$h" linux-x86_64 "$(release 7)" $'\n\n\nn\n'
  grep -q "added by rogg install.sh" "$zd/.zshrc" || { fail "$name" "setup: PATH line not added"; return; }
  # The folder holding .zshrc is read-only (the file itself stays writable): no temporary file can go next to it.
  chmod 555 "$zd"
  ZDOTDIR_TEST=$zd run "$h" linux-x86_64 "$(release 7)" $'\n\n' -- --uninstall
  chmod 755 "$zd"
  grep -q 'alias ll="ls -l"' "$zd/.zshrc" && grep -q 'export EDITOR=vim' "$zd/.zshrc" || { fail "$name" "the user's own lines are gone: '$(cat "$zd/.zshrc")'"; return; }
  ! grep -q "added by rogg install.sh" "$zd/.zshrc" || { fail "$name" "PATH line kept"; return; }
  pass "$name"
}

test_uninstall_leaves_copies_outside_the_default_places() {
  local name=${FUNCNAME[0]} h
  h=$(new_home othercopy); mkdir -p "$h/opt/bin"; fake_rogg "$h/opt/bin/rogg" 3
  TEST_PATH="$h/opt/bin:/usr/bin:/bin" run "$h" linux-x86_64 "$(release 7)" $'\n\n' -- --uninstall
  [[ -x $h/opt/bin/rogg ]] || { fail "$name" "deleted a rogg it did not install"; return; }
  has "~/opt/bin/rogg" && has "remove it yourself" || { fail "$name" "did not point out the other copy"; return; }
  pass "$name"
}

test_a_relative_folder_is_refused() {
  local name=${FUNCNAME[0]} h
  h=$(new_home relative)
  run "$h" linux-x86_64 "$(release 7)" $'bin\n\n\n\n'
  [[ $status != 0 ]] && has "absolute" || { fail "$name" "exit $status: $(tail -2 "$work/out")"; return; }
  [[ ! -e $h/.zshrc ]] || { fail "$name" "wrote a PATH line: $(cat "$h/.zshrc")"; return; }
  pass "$name"
}

test_a_folder_naming_the_app_is_refused() {
  local name=${FUNCNAME[0]} h
  h=$(new_home appfolder)
  run "$h" macos-universal "$(release 7)" "1"$'\n'"3"$'\n'"$h/Apps/rogg.app"$'\n\n'
  [[ $status != 0 ]] && has "the folder" || { fail "$name" "exit $status: $(tail -2 "$work/out")"; return; }
  [[ ! -e $h/Apps/rogg.app/rogg.app ]] || { fail "$name" "nested the app"; return; }
  pass "$name"
}

test_another_rogg_earlier_on_path_is_pointed_out() {
  local name=${FUNCNAME[0]} h
  h=$(new_home shadow); mkdir -p "$h/opt/bin"; fake_rogg "$h/opt/bin/rogg" 3
  TEST_PATH="$h/opt/bin:/usr/bin:/bin" run "$h" linux-x86_64 "$(release 7)" $'\n\nn\nn\n'
  [[ $status == 0 ]] || { fail "$name" "exit $status: $(tail -3 "$work/out")"; return; }
  has "~/opt/bin/rogg" && has "build 3" || { fail "$name" "did not say which rogg the shell runs"; return; }
  pass "$name"
}

test_a_release_without_the_icon_skips_the_desktop_entry() {
  local name=${FUNCNAME[0]} h rel s
  h=$(new_home noicon); rel=$work/release-noicon; s=$work/stage-noicon
  rm -rf "$rel" "$s"; mkdir -p "$rel" "$s"
  fake_rogg "$s/rogg" 7
  (cd "$s" && tar -czf "$rel/rogg-linux-x86_64.tar.gz" rogg)
  "$sh" "$root/tools/ci/manifest.sh" "$rel" 7 abc1234 main
  run "$h" linux-x86_64 "$rel" $'\n\nn\n\n'
  [[ $status == 0 ]] || { fail "$name" "exit $status: $(tail -3 "$work/out")"; return; }
  [[ -x $h/.local/bin/rogg ]] || { fail "$name" "binary not installed"; return; }
  [[ ! -e $h/.local/share/applications/rogg.desktop ]] && has "no icon" || { fail "$name" "did not skip the entry with a note"; return; }
  pass "$name"
}

for t in test_main_is_called_last test_fresh_binary_install_adds_path_once test_upgrade_shows_both_builds \
         test_downgrade_is_refused_by_default test_checksum_mismatch_changes_nothing test_mac_app_installs_to_a_folder_with_spaces \
         test_mac_binary_uses_bash_profile_for_bash test_an_old_binary_without_version_shows_unknown_and_upgrades \
         test_unsupported_platform_stops_cleanly test_no_terminal_stops_with_a_message \
         test_linux_desktop_entry_and_uninstall test_path_line_lands_on_its_own_line \
         test_uninstall_keeps_the_rest_of_a_startup_file test_uninstall_leaves_copies_outside_the_default_places \
         test_a_relative_folder_is_refused test_a_folder_naming_the_app_is_refused \
         test_another_rogg_earlier_on_path_is_pointed_out test_a_release_without_the_icon_skips_the_desktop_entry; do
  "$t"
done
(( failures == 0 )) && echo "all install tests passed" || { echo "$failures failed"; exit 1; }
