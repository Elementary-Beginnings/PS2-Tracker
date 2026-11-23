#!/usr/bin/env python3
"""Writes latest.json for the Tauri updater (v2 format) to stdout.

    python3 tools/updater_manifest.py <version> <dist dir> <download base URL> <CHANGELOG.md>

Only platforms with a signature file (*.sig from `tauri build` with
TAURI_SIGNING_PRIVATE_KEY) are listed: the AppImage (linux-x86_64) and the
NSIS setup (windows-x86_64). Notes = the CHANGELOG section of the version,
shortened to 4000 characters.
"""
import datetime
import json
import os
import re
import sys

FILES = {
    "linux-x86_64": "ps2-tracker-x86_64.AppImage",
    "windows-x86_64": "ps2-tracker-x64-setup.exe",
}


def notes(changelog: str, version: str) -> str:
    text = open(changelog, encoding="utf-8").read()
    m = re.search(rf"^## \[{re.escape(version)}\][^\n]*\n(.*?)(?=^## \[|\Z)", text, re.S | re.M)
    body = (m.group(1) if m else "").strip()
    return body[:4000]


def main() -> int:
    version, dist, base, changelog = sys.argv[1:5]
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        print(f"version must be X.Y.Z, got {version}", file=sys.stderr)
        return 1
    platforms = {}
    for target, name in FILES.items():
        sig = os.path.join(dist, name + ".sig")
        if os.path.exists(sig):
            platforms[target] = {
                "signature": open(sig, encoding="utf-8").read().strip(),
                "url": f"{base.rstrip('/')}/{name}",
            }
    if not platforms:
        print("no signature files – builds were not signed", file=sys.stderr)
        return 1
    json.dump(
        {
            "version": version,
            "notes": notes(changelog, version),
            "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "platforms": platforms,
        },
        sys.stdout,
        indent=2,
    )
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
