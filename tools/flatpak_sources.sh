#!/usr/bin/env bash
# Inputs for the offline Flatpak build (workflow .github/workflows/flatpak.yml,
# job "sources", on a normal Ubuntu runner): validates the desktop file and
# writes packaging/flatpak/cargo-sources.json from Cargo.lock with the pinned
# flatpak-cargo-generator. The frontend (dist/) is built before (pnpm build).
# Needs python3 with aiohttp and tomlkit >= 0.11, curl, desktop-file-validate.
set -euo pipefail

# flatpak-builder-tools: commit and SHA-256 of flatpak-cargo-generator.py
FCG_COMMIT="9d86d336a39b9af38b86f114b6ce5820fe260f91"
FCG_SHA256="0a2db6be87d75910facef28ab46d4d6460802e8419ab850d0caa6a364d26b380"

[ -f dist/index.html ] || { echo "dist/ missing - run pnpm build first" >&2; exit 1; }
desktop-file-validate packaging/flatpak/dev.herz.Ps2Tracker.desktop

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
curl -fsSLo "$tmp/fcg.py" \
  "https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/${FCG_COMMIT}/cargo/flatpak-cargo-generator.py"
echo "${FCG_SHA256}  $tmp/fcg.py" | sha256sum -c -
python3 "$tmp/fcg.py" Cargo.lock -o packaging/flatpak/cargo-sources.json
echo "cargo-sources.json: $(python3 -c 'import json;print(len(json.load(open("packaging/flatpak/cargo-sources.json"))))') sources"
