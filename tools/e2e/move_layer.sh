#!/bin/bash
# E2E test of the move layer under Xvfb + xfwm4 (Linux, X11).
#
# A fullscreen window (tools/e2e/grabwin.c) stands in for PlanetSide 2 in mouse
# look: it identifies like the game (WM_CLASS steam_app_218230) and holds the
# pointer with XGrabPointer for 20 s. Checked:
#   1. the layer opens on the game's monitor: focus=granted pointer=grabbed
#   2. the keyboard works while the game holds the pointer (arrows, Tab), and
#      Esc reaches the game instead of cancelling
#   3. after the release: pointer=free, a drag moves the stats overlay
#   4. Enter saves, the layer closes, the real window is placed once
#   5. a second session is cancelled with Esc, the positions stay
#   6. Ctrl+F2 is taken by another client (tools/e2e/grabkey.c, like KWin
#      "Switch to Desktop 2"): the failure is logged and shown on the layer
#   7. while the game holds the pointer, the move mode ends (and saves) with
#      Ctrl+Esc, with --finish-move and after the idle timeout (30 s here)
#
# Needs: Xvfb, xfwm4, xdotool, xwininfo, ImageMagick (import), dbus-daemon,
# gcc with the X11 headers, a debug build (cargo build -p ps2-tracker) and the
# built frontend (pnpm build). The move mode is opened through the CLI
# (--toggle-edit), because Ctrl+F2 is taken on purpose.
#
# Only processes on its own display are stopped at the end, never the desktop.
set -u
D=${E2E_DISPLAY:-:79}
REPO=$(cd "$(dirname "$0")/../.." && pwd)
O=$(mktemp -d /tmp/ps2t-e2e-move.XXXXXX)
cd "$REPO" || exit 1
gcc -O1 -o "$O/grabwin" tools/e2e/grabwin.c -lX11 || exit 1
gcc -O1 -o "$O/grabkey" tools/e2e/grabkey.c -lX11 || exit 1

cleanup() {
  for n in ps2-tracker xfwm4 grabwin grabkey; do
    for p in $(pgrep -x "$n"); do
      tr '\0' '\n' < "/proc/$p/environ" 2>/dev/null | grep -qx "DISPLAY=$D" && kill "$p"
    done
  done
  [ -s "$O/dbus.pid" ] && kill "$(cat "$O/dbus.pid")" 2>/dev/null
  [ -n "${VITE:-}" ] && kill "$VITE" 2>/dev/null
  [ -n "${XVFB:-}" ] && kill "$XVFB" 2>/dev/null
}
trap cleanup EXIT

export DISPLAY=$D
dbus-daemon --session --fork --print-address=1 --print-pid=1 > "$O/dbus.out"
export DBUS_SESSION_BUS_ADDRESS=$(sed -n 1p "$O/dbus.out")
sed -n 2p "$O/dbus.out" > "$O/dbus.pid"
pnpm exec vite preview --port 1420 --strictPort > "$O/vite.log" 2>&1 & VITE=$!
Xvfb "$D" -screen 0 1920x1080x24 > "$O/xvfb.log" 2>&1 & XVFB=$!
sleep 2
xfwm4 --compositor=off > "$O/xfwm4.log" 2>&1 &
"$O/grabkey" F2 600 > "$O/grabkey.log" 2>&1 &
sleep 1
T=$O/home
mkdir -p "$T/config/ps2-tracker"
cat > "$T/config/ps2-tracker/config.toml" <<EOF
[overlay]
enabled = true
anchor = "top-right"
x = 24
y = 24
move_idle_timeout_secs = 30
EOF
env XDG_CONFIG_HOME="$T/config" XDG_DATA_HOME="$T/data" XDG_STATE_HOME="$T/state" \
  PS2_TRACKER_NO_DMABUF_FIX=1 RUST_LOG=info,ps2_tracker=debug \
  ./target/debug/ps2-tracker > "$O/stdout.log" 2>&1 &
cli() { env XDG_CONFIG_HOME="$T/config" XDG_DATA_HOME="$T/data" XDG_STATE_HOME="$T/state" timeout 15 ./target/debug/ps2-tracker "$@" > /dev/null 2>&1; }
L=$T/state/ps2-tracker/ps2-tracker.log
fail=0
expect() { if grep -q "$2" "$L"; then echo "ok   $1"; else echo "FAIL $1 (no \"$2\" in the log)"; fail=1; fi; }
sleep 9

"$O/grabwin" 0 0 1920 1080 20 90 > "$O/grabwin.log" 2>&1 &
sleep 2
cli --toggle-edit; sleep 4
expect "layer opens while the game holds the pointer" 'move layer opened.*focus="granted" pointer="grabbed"'
import -window root "$O/layer-grabbed.png"
for i in 1 2 3 4 5; do xdotool key Right; sleep 0.2; done
for i in 1 2 3; do xdotool key Down; sleep 0.2; done
xdotool key Tab; sleep 0.3; xdotool key Tab; sleep 0.3
xdotool key Escape; sleep 1
[ "$(grep -c 'hotkey pressed' "$L")" -ge 10 ] && echo "ok   keyboard works during the grab" || { echo "FAIL keyboard during the grab"; fail=1; }
grep -q 'move layer cancelled' "$L" && { echo "FAIL Esc cancelled although the game held the pointer"; fail=1; } || echo "ok   Esc reaches the game during the grab"
sleep 15
expect "pointer free after the release" 'move layer pointer changed pointer="free"'
# stats object after the keys: top-right offset 19,27, content 336 px wide
xdotool mousemove $((1565 + 120)) $((27 + 120)); sleep 0.3; xdotool mousedown 1; sleep 0.2
for i in $(seq 1 30); do xdotool mousemove_relative -- -20 10; sleep 0.03; done
sleep 0.2; import -window root "$O/layer-dragging.png"
xdotool mouseup 1; sleep 0.5
xdotool key Return; sleep 3
expect "Enter saves the dragged position" 'move layer saved stats=top-right 619,327'
expect "real window placed once and checked" 'position check object="stats" requested=965,327 actual=965,327 ok=true'
cli --toggle-edit; sleep 4
xdotool mousemove 1000 400; xdotool mousedown 1
for i in $(seq 1 10); do xdotool mousemove_relative -- 10 10; sleep 0.03; done
xdotool mouseup 1; sleep 0.5
xdotool key Escape; sleep 3
expect "Esc cancels and keeps the positions" 'move layer cancelled, old positions kept stats=top-right 619,327'

# exits while the game holds the pointer; Ctrl+F2 is held by grabkey
expect "a taken Ctrl+F2 is logged" 'hotkey registered failed hotkey=Ctrl+F2'
mark() { M=$(wc -l < "$L"); }
since() { if tail -n +$((M + 1)) "$L" | grep -q "$2"; then echo "ok   $1"; else echo "FAIL $1 (no \"$2\" after the step)"; fail=1; fi; }
"$O/grabwin" 0 0 1920 1080 75 5 > "$O/grabwin2.log" 2>&1 &
sleep 2
mark; cli --toggle-edit; sleep 3
since "layer opens during the second grab" 'move layer opened.*pointer="grabbed"'
import -window root "$O/layer-key-taken.png"
xdotool key ctrl+Escape; sleep 2
since "Ctrl+Esc saves and ends during the grab" 'move layer finished via="hotkey" saved=true'
mark; cli --toggle-edit; sleep 3
cli --finish-move; sleep 2
since "--finish-move saves and ends during the grab" 'move layer finished via="cli" saved=true'
mark; cli --toggle-edit; sleep 3
since "layer open again for the timeout" 'move layer opened.*pointer="grabbed"'
sleep 34
since "the idle timeout (30 s) saves and ends during the grab" 'move layer finished via="timeout" saved=true'
import -window root "$O/after-timeout.png"
[ "$(grep -c 'PS2 Session Tracker starting' "$L")" = 1 ] && echo "ok   CLI calls kept the log" || { echo "FAIL log rotated by a CLI call"; fail=1; }
echo "logs and screenshots: $O"
exit $fail
