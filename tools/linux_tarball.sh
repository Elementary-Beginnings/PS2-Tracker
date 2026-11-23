#!/usr/bin/env bash
# Packs the Linux release binary with its data files into a tarball (top folder
# ps2-tracker/, usr/ tree as in the Arch package). Used by the CI job
# linux-build; the Arch package and the AUR package ps2-tracker-bin install it.
#   tools/linux_tarball.sh <binary> <out.tar.gz>
set -euo pipefail
bin="$1"
out="$2"
root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
d="$tmp/ps2-tracker"
install -Dm755 "$bin" "$d/usr/bin/ps2-tracker"
install -Dm644 "$root/packaging/ps2-tracker.desktop" "$d/usr/share/applications/ps2-tracker.desktop"
install -Dm644 "$root/src-tauri/icons/32x32.png" "$d/usr/share/icons/hicolor/32x32/apps/ps2-tracker.png"
install -Dm644 "$root/src-tauri/icons/128x128.png" "$d/usr/share/icons/hicolor/128x128/apps/ps2-tracker.png"
install -Dm644 "$root/src-tauri/icons/128x128@2x.png" "$d/usr/share/icons/hicolor/256x256/apps/ps2-tracker.png"
install -Dm644 "$root/packaging/kde/ps2-tracker.kwinrule" "$d/usr/share/ps2-tracker/ps2-tracker.kwinrule"
install -Dm644 "$root/crates/ps2-tracker-core/assets/triggers.default.toml" "$d/usr/share/ps2-tracker/triggers.default.toml"
install -Dm644 "$root/crates/ps2-tracker-core/assets/achievements.default.toml" "$d/usr/share/ps2-tracker/achievements.default.toml"
# marks the install as package-managed: the in-app updater stays off
install -Dm644 /dev/null "$d/usr/share/ps2-tracker/package-managed"
cp "$root/LICENSE-MIT" "$root/LICENSE-APACHE" "$root/THIRD-PARTY-NOTICES.md" "$d/"
mkdir -p "$(dirname "$out")"
# reproducible: fixed owner, order and timestamps
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@${SOURCE_DATE_EPOCH:-0}" \
  -C "$tmp" -czf "$out" ps2-tracker
rm -rf "$tmp"
echo "$out"
