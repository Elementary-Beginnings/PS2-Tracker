#!/usr/bin/env python3
"""Release text for GitHub: a short intro plus the CHANGELOG section of the
version. Fails if the section is missing or empty.

    python3 -I tools/release_notes.py <version> CHANGELOG.md > release-notes.md
"""
import re
import sys


def main() -> int:
    version, changelog = sys.argv[1], sys.argv[2]
    text = open(changelog, encoding="utf-8").read()
    m = re.search(rf"^## \[{re.escape(version)}\][^\n]*\n(.*?)(?=^## \[|\Z)", text, re.S | re.M)
    body = (m.group(1) if m else "").strip()
    if not body:
        print(f"CHANGELOG.md: no section for {version}", file=sys.stderr)
        return 1
    print(
        "Downloads below; verify them with `SHA256SUMS` and, optionally, with "
        "`gh attestation verify <file> --repo Elementary-Beginnings/ps2-tracker`. "
        "The Windows setup is not code-signed (SmartScreen warns).\n"
    )
    print(body)
    return 0


if __name__ == "__main__":
    sys.exit(main())
