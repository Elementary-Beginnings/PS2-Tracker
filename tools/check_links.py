#!/usr/bin/env python3
# Used in CI (.github/workflows/ci.yml): python3 tools/check_links.py .
"""Check relative links and anchors in all tracked Markdown files."""
import re, subprocess, sys, os, unicodedata

root = sys.argv[1]
os.chdir(root)
files = subprocess.run(["git", "ls-files", "-co", "--exclude-standard", "*.md"],
                       capture_output=True, text=True).stdout.split()
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

def anchors(path):
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
    return res

bad = 0
link_re = re.compile(r"(?<!!)\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)|!\[[^\]]*\]\(([^)\s]+)\)")
for f in files:
    incode = False
    for no, line in enumerate(open(f, encoding="utf-8"), 1):
        if line.startswith("```"):
            incode = not incode
            continue
        if incode:
            continue
        for m in link_re.finditer(line):
            t = m.group(1) or m.group(2)
            if re.match(r"^[a-z]+:", t) or t.startswith("<"):
                continue
            path, _, frag = t.partition("#")
            target = os.path.normpath(os.path.join(os.path.dirname(f), path)) if path else f
            if not os.path.exists(target):
                print(f"{f}:{no}: missing {t}"); bad += 1; continue
            if frag and target.endswith(".md") and frag not in anchors(target):
                print(f"{f}:{no}: missing anchor {t}"); bad += 1
print(f"{len(files)} files, {bad} broken")
sys.exit(1 if bad else 0)
