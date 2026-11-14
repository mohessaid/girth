"""Grades .claude/skills/girth-sql-migrations/SKILL.md against the authoring rules."""
import re, sys
p = ".claude/skills/girth-sql-migrations/SKILL.md"
try:
    t = open(p).read()
except OSError:
    sys.exit("no file")
m = re.match(r"---\n(.*?)\n---\n(.*)", t, re.S)
if not m:
    sys.exit("no frontmatter")
fm, body = m.groups()
name = re.search(r"^name:\s*(.+)$", fm, re.M)
desc = re.search(r"^description:\s*(.+)$", fm, re.M)
fails = []
if not name or name.group(1).strip().strip('"\'') != "girth-sql-migrations":
    fails.append("name")
d = desc.group(1).strip().strip('"\'') if desc else ""
if not d.startswith("Use when"):
    fails.append("description must start 'Use when'")
if len(d) > 500:
    fails.append("description over 500 chars")
# A workflow summary in the trigger: step words or an arrow/dash chain.
if re.search(r"\b(then|first|step|write .* and .* and)\b|→|->", d, re.I):
    fails.append("description summarises the workflow")
if len(body.split()) > 500:
    fails.append(f"body {len(body.split())} words")
print(fails or "ok")
sys.exit(1 if fails else 0)
