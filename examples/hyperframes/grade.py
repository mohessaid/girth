"""Mechanical grade of one replication run's ./promo/: HyperFrames' own check, read from its JSON.

Usage: grade.py <project-dir> <out.json>
Pass = `hyperframes check` exits 0 with 0 errors; warnings are counted (the 2026-10-14 run
compared them) but don't fail a run. Counts are summed over every section of the report.
"""
import json
import subprocess
import sys
from pathlib import Path

project, out = Path(sys.argv[1]), Path(sys.argv[2])
promo = project / "promo"
g = {"composition": (promo / "index.html").exists()}


def total(node, key):
    if isinstance(node, dict):
        return node.get(key, 0) + sum(total(v, key) for k, v in node.items() if k != key)
    return sum(total(v, key) for v in node) if isinstance(node, list) else 0


if g["composition"]:
    try:
        r = subprocess.run(["npx", "-y", "hyperframes", "check", "--json"], cwd=promo,
                           capture_output=True, text=True, timeout=600)
        g["check_exit"] = r.returncode
        rep, _ = json.JSONDecoder().raw_decode(r.stdout[r.stdout.index("{"):])
        g["errors"], g["warnings"] = total(rep, "errorCount"), total(rep, "warningCount")
    except (OSError, ValueError, subprocess.TimeoutExpired):
        g.setdefault("check_exit", None)
        g["errors"] = g["warnings"] = None
g["pass"] = bool(g.get("check_exit") == 0 and g.get("errors") == 0)
out.write_text(json.dumps(g, indent=1))
print(json.dumps(g))
