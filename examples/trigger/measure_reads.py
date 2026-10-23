"""Bytes Claude pulled from installed skills per crafting session: whole-file reads vs `girth show`.

  measure_reads.py <transcripts-dir>...
Crafted girth-* skills are excluded (reading back your own craft is not waste).
Counts tool_result bytes for Bash cat/sed/head of a file under .claude/skills or .agents/skills,
Read tool calls on such files, and `girth show` calls. Also tallies which files were read whole.
"""
import collections
import json
import re
import sys
from pathlib import Path

SKILL_PATH = re.compile(r"(/\.(?:claude|agents)/skills/(?!girth-)[^\s\"';|&]+)")
whole, shows, sessions, files = [], [], 0, collections.Counter()
for d in sys.argv[1:]:
    for f in sorted(Path(d).glob("*.jsonl")):
        sessions += 1
        pending, w, s, fresh = {}, 0, 0, 0
        for line in f.open():
            try:
                ev = json.loads(line)
            except ValueError:
                continue
            if ev.get("type") == "result":
                mu = ev.get("modelUsage", {}).values()
                fresh = sum(m.get("inputTokens", 0) + m.get("cacheCreationInputTokens", 0) for m in mu)
            msg = ev.get("message") if isinstance(ev.get("message"), dict) else {}
            for c in msg.get("content") if isinstance(msg.get("content"), list) else []:
                if not isinstance(c, dict):
                    continue
                if c.get("type") == "tool_use":
                    inp = c.get("input", {})
                    text = inp.get("command", "") if c.get("name") == "Bash" else inp.get("file_path", "")
                    if "girth show" in text:
                        pending[c["id"]] = ("show", [])
                    elif c.get("name") == "Read" and SKILL_PATH.search(text):
                        pending[c["id"]] = ("whole", SKILL_PATH.findall(text))
                    elif c.get("name") == "Bash" and re.search(r"\b(cat|sed|head|tail)\b", text) and SKILL_PATH.search(text):
                        pending[c["id"]] = ("whole", SKILL_PATH.findall(text))
                elif c.get("type") == "tool_result" and c.get("tool_use_id") in pending:
                    kind, paths = pending.pop(c["tool_use_id"])
                    content = c.get("content")
                    n = len(json.dumps(content)) if not isinstance(content, str) else len(content)
                    if kind == "show":
                        s += n
                    else:
                        w += n
                        files.update(p.split("/skills/", 1)[1] for p in paths)
        whole.append((w, fresh))
        shows.append(s)
tw, ts = sum(w for w, _ in whole), sum(shows)
tf = sum(f for _, f in whole)
print(f"{sessions} sessions: whole-file reads {tw/1000:.0f} KB (~{tw/4/sessions/1000:.1f}K tok/session), "
      f"girth show {ts/1000:.0f} KB; whole reads ≈ {100*tw/4/tf:.0f}% of fresh input")
for name, n in files.most_common(8):
    print(f"  {n:3d}  {name}")
