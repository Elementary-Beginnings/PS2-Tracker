#!/usr/bin/env python3
# Used in CI (.github/workflows/ci.yml): python3 -I tools/check_links.py .
"""Check links and anchors in all tracked Markdown files, including the wiki.

- Relative links (and <img src>) must point to an existing file; anchors in
  Markdown targets must exist.
- Pages in docs/wiki/ are published to the GitHub wiki, where only page names
  resolve: relative links there must be page names (`Page-Name`,
  `Page-Name#anchor` or `[[Page Name]]`), and docs/wiki/<Page-Name>.md must
  exist. Repository files are linked with absolute URLs.
- Absolute URLs into this repository are checked against the working tree:
  github.com/<repo>/blob|tree/main/<path>, raw.githubusercontent.com/<repo>/main/<path>
  and github.com/<repo>/wiki/<Page-Name>. Other external URLs are not fetched.
"""
import os
import re
import subprocess
import sys

REPO = "Elementary-Beginnings/ps2-tracker"
WIKI_DIR = "docs/wiki"

root = sys.argv[1]
os.chdir(root)
files = subprocess.run(["git", "ls-files", "-co", "--exclude-standard", "*.md"],
                       capture_output=True, text=True, check=True).stdout.split()
files = [f for f in files if not f.startswith("node_modules/") and os.path.exists(f)]


def slug(h):
    h = h.strip().lower()
    h = re.sub(r"[`*_~]", "", h)
    h = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", h)
    out = []
    for ch in h:
        if ch.isalnum() or ch in "-_":
            out.append(ch)
        elif ch == " ":
            out.append("-")
    return "".join(out)


_anchor_cache = {}


def anchors(path):
    if path in _anchor_cache:
        return _anchor_cache[path]
    res, seen = set(), {}
    incode = False
    for line in open(path, encoding="utf-8"):
        if line.startswith("```"):
            incode = not incode
            continue
        if incode:
            continue
        m = re.match(r"^#{1,6}\s+(.*?)\s*#*\s*$", line)
        if m:
            s = slug(m.group(1))
            n = seen.get(s, 0)
            seen[s] = n + 1
            res.add(s if n == 0 else f"{s}-{n}")
        for a in re.findall(r'<a (?:name|id)="([^"]+)"', line):
            res.add(a)
    _anchor_cache[path] = res
    return res


def check_file(target, frag):
    """None if fine, else the problem."""
    if not os.path.exists(target):
        return "missing"
    if frag and target.endswith(".md") and frag not in anchors(target):
        return "missing anchor"
    return None


def wiki_page(name, frag):
    if name in ("", "Home"):
        name = "Home"
    if "/" in name or name.endswith(".md"):
        return "not a wiki page name"
    return check_file(os.path.join(WIKI_DIR, name + ".md"), frag)


ABS = [
    (re.compile(rf"^https://github\.com/{re.escape(REPO)}/(?:blob|tree)/main/([^#?]*)(?:#(.*))?$"),
     lambda m: check_file(os.path.normpath(m.group(1) or "."), m.group(2))),
    (re.compile(rf"^https://raw\.githubusercontent\.com/{re.escape(REPO)}/main/([^#?]+)$"),
     lambda m: check_file(os.path.normpath(m.group(1)), None)),
    (re.compile(rf"^https://github\.com/{re.escape(REPO)}/wiki/?([^#?]*)(?:#(.*))?$"),
     lambda m: wiki_page(m.group(1), m.group(2))),
]

link_re = re.compile(r"\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
img_re = re.compile(r'<img [^>]*src="([^"]+)"')
wikilink_re = re.compile(r"\[\[([^\]|]+)(?:\|[^\]]*)?\]\]")
autolink_re = re.compile(r"<(https://[^>\s]+)>")

bad = 0
for f in files:
    in_wiki = f.startswith(WIKI_DIR + "/")
    incode = False
    for no, line in enumerate(open(f, encoding="utf-8"), 1):
        if line.startswith("```"):
            incode = not incode
            continue
        if incode:
            continue
        line_nocode = re.sub(r"`[^`]*`", "", line)
        targets = [m.group(1) for m in link_re.finditer(line_nocode)]
        targets += [m.group(1) for m in img_re.finditer(line_nocode)]
        targets += [m.group(1) for m in autolink_re.finditer(line_nocode)]
        problems = []
        for m in wikilink_re.finditer(line_nocode):
            page, _, frag = m.group(1).strip().partition("#")
            p = wiki_page(page.replace(" ", "-"), frag) if in_wiki else "wiki link outside docs/wiki"
            if p:
                problems.append((p, m.group(0)))
        for t in targets:
            p = None
            if re.match(r"^[a-z]+:", t):
                for rx, fn in ABS:
                    m = rx.match(t)
                    if m:
                        p = fn(m)
                        break
            elif in_wiki:
                path, _, frag = t.partition("#")
                p = wiki_page(path, frag) if path else (
                    None if not frag or frag in anchors(f) else "missing anchor")
            else:
                path, _, frag = t.partition("#")
                target = os.path.normpath(os.path.join(os.path.dirname(f), path)) if path else f
                p = check_file(target, frag)
            if p:
                problems.append((p, t))
        for p, t in problems:
            print(f"{f}:{no}: {p}: {t}")
            bad += 1
print(f"{len(files)} files, {bad} broken")
sys.exit(1 if bad else 0)
