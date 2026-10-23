"""Summarise trigger-run transcripts (stream-json) into small per-run JSON, plus a table.

  summarize.py <transcripts-dir> <out-dir> <workdir>
Transcripts carry third-party skill text and local paths, so only these summaries are kept.
fired = the girth skill was invoked; crafted = a girth-* skill exists in the run's project;
denied = a write was refused for permission; whole_reads = Bash cat/sed of an installed skill file;
shows = `girth show` calls; validated = `girth validate` calls.
"""
import json
import sys
from pathlib import Path

src, out, work = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
out.mkdir(parents=True, exist_ok=True)
rows = []
for f in sorted(src.glob("*.jsonl")):
    r = {"run": f.stem, "fired": False, "denied": 0, "whole_reads": 0, "shows": 0, "validated": 0, "cost": None}
    for line in f.open():
        try:
            d = json.loads(line)
        except ValueError:
            continue
        if d.get("type") == "result":
            r["cost"] = round(d.get("total_cost_usd", 0), 4)
        msg = d.get("message") if isinstance(d.get("message"), dict) else {}
        content = msg.get("content") if isinstance(msg.get("content"), list) else []
        for c in content:
            if not isinstance(c, dict):
                continue
            if c.get("type") == "tool_use":
                inp = json.dumps(c.get("input", {}))
                if c.get("name") == "Skill" and "girth:girth" in inp:
                    r["fired"] = True
                if c.get("name") == "Bash" and ("cat " in inp or "sed -n" in inp) and ".claude/skills" in inp:
                    r["whole_reads"] += 1
                r["shows"] += "girth show" in inp
                r["validated"] += "girth validate" in inp
            if c.get("type") == "tool_result" and "requested permissions to write" in str(c.get("content")):
                r["denied"] += 1
    skills = work / f.stem / ".claude" / "skills"
    r["crafted"] = skills.is_dir() and any(p.name.startswith("girth-") for p in skills.iterdir())
    (out / f"{f.stem}.json").write_text(json.dumps(r))
    rows.append(r)
n = len(rows)
print(f"runs {n}: fired {sum(r['fired'] for r in rows)}, crafted {sum(r['crafted'] for r in rows)}, "
      f"denied {sum(r['denied'] > 0 for r in rows)}, whole-file reads in {sum(r['whole_reads'] > 0 for r in rows)}, "
      f"girth show in {sum(r['shows'] > 0 for r in rows)}, cost ${sum(r['cost'] or 0 for r in rows):.2f}")
