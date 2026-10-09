#!/usr/bin/env bash
# Starts a virtual X display with a compositing window manager (gpui only draws client decorations when a compositor
# and _GTK_FRAME_EXTENTS support are present), runs rogg (or a gallery page) with software Vulkan, captures the
# window, then quits. Any failure (no X, no WM, dead app, blank render) exits non-zero with a message.
# Usage: tools/linux/shot.sh demo | <gallery-page>
set -euo pipefail
what=${1:-demo}

# Rendered check: the window interior (inset 10 px from every edge, so frames, borders and a cursor at the edge
# don't count) must be neither black nor uniform: mean > 0.02 and standard deviation > 0.02 (both 0..1).
# Prints "mean stddev"; exits non-zero when the image is not a real render.
measure() {
  local m
  m=$(convert "$1" -shave 10x10 -format '%[fx:mean] %[fx:standard_deviation]' info:)
  echo "$m"
  awk -v m="$m" 'BEGIN { split(m, a, " "); exit !(a[1] > 0.02 && a[2] > 0.02) }'
}
if [[ $what == measure ]]; then measure "$2"; exit; fi
[[ $what =~ ^[A-Za-z0-9_-]+$ ]] || { echo "shot.sh: bad page name '$what'" >&2; exit 2; }
out=/work/target/linux/shots
fb=/tmp/xvfb-fb
mkdir -p "$out" "$fb"
export DISPLAY=:99
export XDG_RUNTIME_DIR=/tmp/xdg-runtime
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"

pids=()
cleanup() {
  local p alive
  for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
  for _ in $(seq 25); do
    alive=0
    for p in "${pids[@]}"; do if kill -0 "$p" 2>/dev/null; then alive=1; fi; done
    [[ $alive == 0 ]] && return 0
    sleep 0.2
  done
  for p in "${pids[@]}"; do kill -9 "$p" 2>/dev/null || true; done
}
trap cleanup EXIT
fail() { echo "shot.sh: $*" >&2; exit 1; }

# The screen is 1920x1200 rather than the spec's 1440x900 because the gallery window is 1640x1000.
# -fbdir keeps the screen as an XWD file: with a compositor running, `import -window root` returns black, but the
# framebuffer holds what the compositor painted.
Xvfb :99 -screen 0 1920x1200x24 -nolisten tcp -fbdir "$fb" &
pids+=($!)
up=
for _ in $(seq 50); do
  if xdpyinfo >/dev/null 2>&1; then up=1; break; fi
  sleep 0.1
done
[[ -n $up ]] || fail "Xvfb did not come up on $DISPLAY"

xfwm4 --compositor=on >"$out/wm.log" 2>&1 &
wm=$!
pids+=("$wm")
wm_up=
for _ in $(seq 50); do
  kill -0 "$wm" 2>/dev/null || fail "xfwm4 exited; see $out/wm.log"
  if xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null | grep -q 'window id'; then wm_up=1; break; fi
  sleep 0.2
done
[[ -n $wm_up ]] || fail "xfwm4 never became the window manager; see $out/wm.log"

if [[ $what == demo ]]; then
  cargo build -p rogg-app
  /work/target/linux/debug/rogg --demo >"$out/app.log" 2>&1 &
else
  cargo build -p rogg-gallery
  /work/target/linux/debug/rogg-gallery --page "$what" --chrome linux >"$out/app.log" 2>&1 &
fi
app=$!
pids+=("$app")

# Wait for the app's window (by PID, wider than 300 px so helper windows never match).
win=
for _ in $(seq 150); do
  kill -0 "$app" 2>/dev/null || { tail -n 20 "$out/app.log" >&2; fail "the app exited before showing a window"; }
  for w in $(xdotool search --onlyvisible --pid "$app" 2>/dev/null || true); do
    width=$(xdotool getwindowgeometry "$w" | awk '/Geometry/ {split($2, a, "x"); print a[1]}')
    if [[ ${width:-0} -gt 300 ]]; then win=$w; break; fi
  done
  [[ -n $win ]] && break
  sleep 0.2
done
[[ -n $win ]] || { tail -n 20 "$out/app.log" >&2; fail "no app window appeared; see $out/app.log"; }

# The window geometry includes the client-side shadow margin (_GTK_FRAME_EXTENTS: left, right, top, bottom).
geometry=$(xdotool getwindowgeometry --shell "$win") || fail "cannot read the geometry of window $win"
eval "$geometry"
extents=$(xprop -id "$win" _GTK_FRAME_EXTENTS | sed -n 's/.*= *//p' | tr -d ',' || true)
read -r el er et eb <<<"${extents:-0 0 0 0}"
echo "window $win at $X,$Y ${WIDTH}x$HEIGHT, frame extents: $el $er $et $eb"
if [[ $what == demo && $el -eq 0 ]]; then
  fail "the demo window has no client decorations (no _GTK_FRAME_EXTENTS); the compositor or WM is not working, see $out/wm.log"
fi

# Writes the cropped window to $1 once it is not a uniform image (and, with $2, differs from that image).
shoot() {
  local dst=$1 other=${2:-} diff
  for _ in $(seq 100); do
    convert "xwd:$fb/Xvfb_screen0" -crop "${WIDTH}x${HEIGHT}+${X}+${Y}" +repage "$dst"
    if measure "$dst" >/dev/null; then
      if [[ -z $other ]]; then return 0; fi
      diff=$(compare -metric AE "$dst" "$other" null: 2>&1 || true)
      if [[ $diff != 0 ]]; then return 0; fi
    fi
    sleep 0.2
  done
  echo "last measurement (mean stddev): $(measure "$dst" || true)" >&2
  fail "$dst stayed blank${other:+ or identical to $other} for 20 s; see $out/app.log"
}

xdotool mousemove 1919 1199   # park the cursor in the screen corner, away from the window
shoot "$out/$what.png"
echo "$what.png interior (mean stddev): $(measure "$out/$what.png")"
if [[ $what == demo ]]; then
  # Open the ☰ menu: the app-menu button is the first element of the title bar's leading block (13 px padding plus
  # half of its 25 px width from the content's left edge, half the 42 px title bar down), inside the shadow margin.
  xdotool mousemove $((X + el + 13 + 12)) $((Y + et + 21)) click 1
  shoot "$out/demo-menu.png" "$out/demo.png"
fi
echo "shots in target/linux/shots"
