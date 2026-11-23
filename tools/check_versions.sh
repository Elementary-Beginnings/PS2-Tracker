#!/bin/bash
# Fails when the version files disagree, or (on a tag pipeline) when the tag
# is not v<version>. Source of truth: [workspace.package] in Cargo.toml.
# Run in CI (job check) and by tools/release.sh after setting the version.
set -euo pipefail
cd "$(dirname "$0")/.."
python3 -I - "${CI_COMMIT_TAG:-${1:-}}" <<'PY'
import json, re, sys
tag = sys.argv[1]
def first(path, pattern):
    m = re.search(pattern, open(path, encoding="utf-8").read(), flags=re.M)
    return m.group(1) if m else None
v = first("Cargo.toml", r'^version = "([^"]+)"')
found = {
    "Cargo.toml": v,
    "package.json": json.load(open("package.json"))["version"],
    "src-tauri/tauri.conf.json": json.load(open("src-tauri/tauri.conf.json")).get("version", v),
    "packaging/arch/PKGBUILD": first("packaging/arch/PKGBUILD", r"^pkgver=(\S+)"),
    "packaging/aur/PKGBUILD": first("packaging/aur/PKGBUILD", r"^pkgver=(\S+)"),
    "Flatpak metainfo (newest <release>)": first(
        "packaging/flatpak/dev.herz.Ps2Tracker.metainfo.xml", r'<release version="([^"]+)"'),
}
lock = open("Cargo.lock", encoding="utf-8").read()
for name in ("ps2-tracker", "ps2-tracker-core"):
    m = re.search(rf'name = "{name}"\nversion = "([^"]+)"', lock)
    found[f"Cargo.lock ({name})"] = m.group(1) if m else None
bad = {k: x for k, x in found.items() if x != v}
for k, x in found.items():
    print(f"{'ok ' if x == v else 'BAD'} {k}: {x}")
if bad:
    sys.exit(f"version mismatch: expected {v} everywhere")
if tag and tag != f"v{v}":
    sys.exit(f"tag {tag} does not match the version {v} (expected v{v})")
print(f"version {v} consistent" + (f", tag {tag} ok" if tag else ""))
PY
