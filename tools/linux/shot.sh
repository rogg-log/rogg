#!/usr/bin/env bash
# Starts a virtual X display with a compositing window manager (gpui only draws client decorations when a compositor
# and _GTK_FRAME_EXTENTS support are present), runs rogg (or a gallery page) with software Vulkan, captures the
# window, then quits.
# Usage: tools/linux/shot.sh demo | <gallery-page>
set -euo pipefail
what=${1:-demo}
out=/work/target/linux/shots
fb=/tmp/xvfb-fb
mkdir -p "$out" "$fb"
export DISPLAY=:99
export XDG_RUNTIME_DIR=/tmp/xdg-runtime
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
# -fbdir keeps the screen as an XWD file: with a compositor running, `import -window root` returns black, but the
# framebuffer holds what the compositor painted.
Xvfb :99 -screen 0 1920x1200x24 -nolisten tcp -fbdir "$fb" &
pids=($!)
trap 'kill "${pids[@]}" 2>/dev/null || true' EXIT
for _ in $(seq 50); do xdpyinfo >/dev/null 2>&1 && break; sleep 0.1; done

xfwm4 --compositor=on >"$out/wm.log" 2>&1 &
pids+=($!)
sleep 2

if [[ $what == demo ]]; then
  cargo build -p rogg-app
  /work/target/linux/debug/rogg --demo >"$out/app.log" 2>&1 &
else
  cargo build -p rogg-gallery
  /work/target/linux/debug/rogg-gallery --page "$what" --chrome linux >"$out/app.log" 2>&1 &
fi
app=$!

# Wait for the app's window (by PID, wider than 300 px so helper windows never match).
win=
for _ in $(seq 100); do
  for w in $(xdotool search --onlyvisible --pid "$app" 2>/dev/null || true); do
    width=$(xdotool getwindowgeometry "$w" | awk '/Geometry/ {split($2, a, "x"); print a[1]}')
    if [[ ${width:-0} -gt 300 ]]; then win=$w; break; fi
  done
  [[ -n $win ]] && break
  sleep 0.2
done
[[ -n $win ]] || { echo "no app window appeared; see $out/app.log" >&2; exit 1; }
sleep 4

# The window geometry includes the client-side shadow margin (_GTK_FRAME_EXTENTS: left, right, top, bottom).
eval "$(xdotool getwindowgeometry --shell "$win")"
extents=$(xprop -id "$win" _GTK_FRAME_EXTENTS | sed -n 's/.*= *//p' | tr -d ',' || true)
read -r el er et eb <<<"${extents:-0 0 0 0}"
echo "window $win at $X,$Y ${WIDTH}x$HEIGHT, frame extents: $el $er $et $eb"
shoot() { convert "xwd:$fb/Xvfb_screen0" -crop "${WIDTH}x${HEIGHT}+${X}+${Y}" +repage "$1"; }

shoot "$out/$what.png"
if [[ $what == demo ]]; then
  # Open the ☰ menu: the app-menu button is the first element of the title bar's leading block (13 px padding plus
  # half of its 25 px width from the content's left edge, half the 42 px title bar down), inside the shadow margin.
  xdotool mousemove $((X + el + 13 + 12)) $((Y + et + 21)) click 1
  sleep 1
  shoot "$out/demo-menu.png"
fi
kill "$app" 2>/dev/null || true
wait "$app" 2>/dev/null || true
echo "shots in target/linux/shots"
