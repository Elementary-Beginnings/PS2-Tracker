#!/bin/bash
# E2E test: a squad member added at runtime is subscribed on the open ESS
# connection and appears in the overlay/OBS snapshot right away.
#
# tools/fake_ess.py stands in for the event stream, the OBS endpoint delivers
# the same snapshots as the overlay window. The member is added with the
# debug-only CLI --debug-squad-add, which runs the same code as the Squad page
# (save_config) without a Census lookup. Checked:
#   1. the fake ESS receives a "subscribe" with the new ID on the same connection
#   2. the app logs "ESS subscription updated added=1", no reconnect
#   3. the SSE stream of the OBS endpoint pushes a snapshot with the member
#      (Squad top 3, 0 kills, listed although below the minimum kills)
#   4. classes from the synthetic loadouts: kill feed rows carry the enemy's
#      class, Squad top 3 the member's current class
#
# Needs: Xvfb, xfwm4, ImageMagick (import), dbus-daemon, curl, python3 with
# "websockets", a debug build (cargo build -p ps2-tracker) and pnpm build.
# Only processes on its own display are stopped at the end.
set -u
D=${E2E_DISPLAY:-:78}
PORT=${E2E_ESS_PORT:-8766}
OBS=${E2E_OBS_PORT:-47901}
REPO=$(cd "$(dirname "$0")/../.." && pwd)
O=$(mktemp -d /tmp/ps2t-e2e-squad.XXXXXX)
cd "$REPO" || exit 1
OWN=5428000000000000042
M1=5428000000000000077
NEW=5428000000000000099

cleanup() {
  for n in ps2-tracker xfwm4; do
    for p in $(pgrep -x "$n"); do
      tr '\0' '\n' < "/proc/$p/environ" 2>/dev/null | grep -qx "DISPLAY=$D" && kill "$p"
    done
  done
  for v in FAKE SSE VITE XVFB; do [ -n "${!v:-}" ] && kill "${!v}" 2>/dev/null; done
  [ -s "$O/dbus.pid" ] && kill "$(cat "$O/dbus.pid")" 2>/dev/null
}
trap cleanup EXIT

export DISPLAY=$D
dbus-daemon --session --fork --print-address=1 --print-pid=1 > "$O/dbus.out"
export DBUS_SESSION_BUS_ADDRESS=$(sed -n 1p "$O/dbus.out")
sed -n 2p "$O/dbus.out" > "$O/dbus.pid"
python3 tools/fake_ess.py --own $OWN --squad $M1 --kills 6 --interval 1 --port "$PORT" > "$O/fake_ess.log" 2>&1 & FAKE=$!
pnpm exec vite preview --port 1420 --strictPort > "$O/vite.log" 2>&1 & VITE=$!
Xvfb "$D" -screen 0 1920x1080x24 > "$O/xvfb.log" 2>&1 & XVFB=$!
sleep 2
xfwm4 --compositor=off > "$O/xfwm4.log" 2>&1 &
sleep 1
T=$O/home
mkdir -p "$T/config/ps2-tracker"
cat > "$T/config/ps2-tracker/config.toml" <<EOF
[[character]]
name = "E2eOwn"
id = "$OWN"
faction_id = 2

[[squad]]
name = "E2eMember"
id = "$M1"
faction_id = 2

[overlay]
enabled = true
show_squad = true
anchor = "top-right"
x = 24
y = 24

[obs]
enabled = true
port = $OBS
EOF
env XDG_CONFIG_HOME="$T/config" XDG_DATA_HOME="$T/data" XDG_STATE_HOME="$T/state" \
  PS2_TRACKER_ESS_URL="ws://127.0.0.1:$PORT" PS2_TRACKER_NO_DMABUF_FIX=1 RUST_LOG=info,ps2_tracker=debug \
  ./target/debug/ps2-tracker > "$O/stdout.log" 2>&1 &
L=$T/state/ps2-tracker/ps2-tracker.log
fail=0
ok() { echo "ok   $1"; }
bad() { echo "FAIL $1"; fail=1; }
sleep 12
curl -sN "http://127.0.0.1:$OBS/api/events" > "$O/sse.log" 2>&1 & SSE=$!
sleep 1
grep -q "$NEW" "$O/sse.log" && bad "new member in the snapshot before it was added"
env XDG_CONFIG_HOME="$T/config" XDG_DATA_HOME="$T/data" XDG_STATE_HOME="$T/state" \
  timeout 15 ./target/debug/ps2-tracker --debug-squad-add $NEW E2eNewMember > /dev/null 2>&1
sleep 4
grep -q "received:.*\"subscribe\".*$NEW" "$O/fake_ess.log" && ok "fake ESS got a subscribe with the new ID" || bad "no subscribe with the new ID at the fake ESS"
[ "$(grep -c '^subscription:' "$O/fake_ess.log")" = 1 ] && ok "same connection, no reconnect" || bad "the app reconnected"
grep -q 'ESS subscription updated.*added=1' "$L" && ok "app logs the live subscription update" || bad "no \"ESS subscription updated added=1\" in the log"
grep -q "\"character_id\":\"$NEW\",\"name\":\"E2eNewMember\"" "$O/sse.log" && ok "OBS/overlay snapshot pushed with the new member" || bad "the new member is not in the pushed snapshot"
curl -s "http://127.0.0.1:$OBS/api/snapshot" | grep -q "E2eNewMember" && ok "current snapshot contains the member" || bad "current snapshot without the member"
grep -q "^id = \"$NEW\"" "$T/config/ps2-tracker/config.toml" && ok "member saved in config.toml" || bad "member not saved"
SNAP=$(curl -s "http://127.0.0.1:$OBS/api/snapshot")
echo "$SNAP" | grep -q '"other_class":"\(medic\|heavy_assault\|infiltrator\|light_assault\|engineer\|max\)"' && ok "kill feed rows carry the enemy class" || bad "no enemy class in the kill feed"
echo "$SNAP" | grep -q '"classes":{"enemy":\[{"class":"' && ok "snapshot has the class evaluation" || bad "no class evaluation in the snapshot"
echo "$SNAP" | grep -q "\"character_id\":\"$M1\"[^}]*\"class\":\"" && ok "Squad top 3 shows the member's class" || bad "no class for the squad member"
import -window root "$O/overlay.png"
echo "logs and screenshot: $O"
exit $fail
