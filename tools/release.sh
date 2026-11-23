#!/bin/bash
# Prepares a release: sets the version everywhere, dates the CHANGELOG
# section "Unreleased", commits and creates an annotated tag vX.Y.Z whose
# message is that CHANGELOG section. Pushes nothing (prints the command).
#
#   tools/release.sh patch|minor|major|X.Y.Z
#
# patch/minor/major count up from the current version in Cargo.toml. At the
# end it asks before pushing main and the tag (non-interactive: only prints).
# Version files: Cargo.toml ([workspace.package]; Cargo.lock follows),
# package.json, src-tauri/tauri.conf.json, packaging/arch/PKGBUILD,
# packaging/aur/PKGBUILD (pkgrel back to 1), Flatpak metainfo (<release>).
# Semver rules: docs/release.md.
set -euo pipefail
cd "$(dirname "$0")/.."
CUR=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)
IFS=. read -r MA MI PA <<<"$CUR"
case "${1:-}" in
  patch) V="$MA.$MI.$((PA + 1))" ;;
  minor) V="$MA.$((MI + 1)).0" ;;
  major) V="$((MA + 1)).0.0" ;;
  *) V=${1:-} ;;
esac
if ! [[ $V =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: $0 patch|minor|major|X.Y.Z (semantic version, no leading v)" >&2
  exit 2
fi
echo "version: $CUR -> $V"
[ -z "$(git status --porcelain)" ] || { echo "working tree not clean" >&2; exit 1; }
[ "$(git rev-parse --abbrev-ref HEAD)" = main ] || { echo "not on main" >&2; exit 1; }
git rev-parse -q --verify "refs/tags/v$V" >/dev/null && { echo "tag v$V exists" >&2; exit 1; }
DATE=$(date +%F)

python3 -I - "$V" "$DATE" <<'EOF'
import json, re, sys
v, date = sys.argv[1], sys.argv[2]

def sub(path, pattern, repl, count=1):
    text = open(path, encoding="utf-8").read()
    new, n = re.subn(pattern, repl, text, count=count, flags=re.M)
    if n == 0:
        sys.exit(f"{path}: pattern not found: {pattern}")
    open(path, "w", encoding="utf-8").write(new)

# [workspace.package] is the first `version = ` in Cargo.toml
sub("Cargo.toml", r'^version = "[^"]*"', f'version = "{v}"')
for p in ("package.json", "src-tauri/tauri.conf.json"):
    sub(p, r'^(  "version": )"[^"]*"', rf'\g<1>"{v}"')
for p in ("packaging/arch/PKGBUILD", "packaging/aur/PKGBUILD"):
    sub(p, r"^pkgver=.*$", f"pkgver={v}")
    sub(p, r"^pkgrel=.*$", "pkgrel=1")

# CHANGELOG: "## [Unreleased]" with content becomes "## [v] – date"
cl = open("CHANGELOG.md", encoding="utf-8").read()
m = re.search(r"^## \[Unreleased\]\n(.*?)(?=^## \[)", cl, flags=re.M | re.S)
if not m or not m.group(1).strip():
    sys.exit("CHANGELOG.md: section [Unreleased] is missing or empty")
if re.search(rf"^## \[{re.escape(v)}\]", cl, flags=re.M):
    sys.exit(f"CHANGELOG.md: section [{v}] exists already")
cl = cl[: m.start()] + f"## [Unreleased]\n\n## [{v}] – {date}\n" + m.group(1) + cl[m.end():]
open("CHANGELOG.md", "w", encoding="utf-8").write(cl)

# Flatpak metainfo: one <release> per version, newest first
mi = "packaging/flatpak/dev.herz.Ps2Tracker.metainfo.xml"
text = open(mi, encoding="utf-8").read()
if re.search(rf'<release version="{re.escape(v)}" date="[^"]*"', text):
    text = re.sub(rf'(<release version="{re.escape(v)}" date=")[^"]*"', rf'\g<1>{date}"', text)
else:
    entry = (f'<release version="{v}" date="{date}">\n'
             f'      <description>\n        <p>See CHANGELOG.md for the changes in {v}.</p>\n'
             f'      </description>\n    </release>\n    ')
    text = text.replace("<releases>\n    ", "<releases>\n    " + entry, 1)
open(mi, "w", encoding="utf-8").write(text)
EOF

# Cargo.lock carries the workspace version
cargo update --workspace --offline -q 2>/dev/null || cargo update --workspace -q
bash tools/check_versions.sh "v$V"

# tag message: short form of the dated CHANGELOG section – its headings and
# the bold lead-in of each entry (the full text goes into the GitHub release)
NOTES=$(python3 -I - "$V" <<'PY'
import re, sys
v = sys.argv[1]
lines = open("CHANGELOG.md", encoding="utf-8").read().splitlines()
start = next(i for i, l in enumerate(lines) if l.startswith(f"## [{v}]"))
out = []
for l in lines[start + 1:]:
    if l.startswith("## ["):
        break
    if l.startswith("### "):
        out += ["", l[4:]]
    elif l.startswith("- "):
        m = re.match(r"- \*\*(.+?)\*\*", l)
        t = m.group(1) if m else l[2:]
        out.append("- " + (t if len(t) <= 72 else t[:69].rstrip() + "…"))
print("\n".join(out))
PY
)
# never tag with an empty message
[ -n "$NOTES" ] || { echo "empty CHANGELOG section" >&2; exit 1; }
git add Cargo.toml Cargo.lock package.json src-tauri/tauri.conf.json \
  packaging/arch/PKGBUILD packaging/aur/PKGBUILD CHANGELOG.md \
  packaging/flatpak/dev.herz.Ps2Tracker.metainfo.xml
# RELEASE_TRAILER (optional): extra paragraph for the commit message
git commit -q -m "Release $V" ${RELEASE_TRAILER:+-m "$RELEASE_TRAILER"}
git tag -a "v$V" -F - <<MSG
PS2 Session Tracker $V ($DATE)
$NOTES
MSG
echo "Release commit and tag v$V created. Check with: git show v$V"
if [ -t 0 ]; then
  read -r -p "Push main and tag v$V now? [y/N] " ok
  if [ "$ok" = y ] || [ "$ok" = Y ]; then
    git push origin main && git push origin "v$V"
    exit 0
  fi
fi
echo "Publish:  git push origin main && git push origin v$V"
