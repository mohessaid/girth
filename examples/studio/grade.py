"""Mechanical grade of one run's ./studio/ project, so no model judges its own output.

Usage: grade.py <project-dir> <out.json>
Pass = check exits 0, the render matches the brief, every hand-off file is valid, and all
six panel labels are in the composition. The pass gate is check's exit status; the error and
warning counts are summed over every section of its report, so a report that also repeats a
total at the top level would count twice (hyperframes 0.8.169 does not).
"""
import json
import re
import subprocess
import sys
from pathlib import Path

project, out = Path(sys.argv[1]), Path(sys.argv[2])
studio = project / "studio"
LABELS = ["Reference", "Data", "Story", "Motion", "Direct", "Ship"]
g = {}


def run(cmd, cwd):
    try:
        return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=600)
    except (OSError, subprocess.TimeoutExpired) as e:
        return subprocess.CompletedProcess(cmd, 99, "", str(e))


# 1. HyperFrames' own gate: exit status plus finding counts from its JSON.
if (studio / "index.html").exists():
    r = run(["npx", "-y", "hyperframes", "check", "--json"], studio)
    g["check_exit"] = r.returncode
    # one JSON report (log lines may follow it); each section carries its own counts
    def total(node, key):
        if isinstance(node, dict):
            return node.get(key, 0) + sum(total(v, key) for k, v in node.items() if k != key)
        return sum(total(v, key) for v in node) if isinstance(node, list) else 0

    try:
        rep, _ = json.JSONDecoder().raw_decode(r.stdout[r.stdout.index("{"):])
        g["check_ok"] = rep.get("ok")
        g["check_errors"] = total(rep, "errorCount")
        g["check_warnings"] = total(rep, "warningCount")
    except ValueError:
        g["check_ok"] = g["check_errors"] = g["check_warnings"] = None
else:
    g["check_exit"] = None

# 2. The render: exists, portrait 1080x1350, 9 s (±0.25), 30 fps.
mp4 = studio / "final.mp4"
g["render"] = mp4.exists() and mp4.stat().st_size > 0
if g["render"]:
    p = run(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
             "stream=width,height,r_frame_rate:format=duration", "-of", "json", str(mp4)], studio)
    info = json.loads(p.stdout or "{}")
    s = (info.get("streams") or [{}])[0]
    num, _, den = s.get("r_frame_rate", "0/1").partition("/")
    g["width"], g["height"] = s.get("width"), s.get("height")
    g["fps"] = round(int(num) / int(den or 1), 2)
    g["duration"] = round(float(info.get("format", {}).get("duration", 0)), 2)
g["render_ok"] = bool(g["render"] and g.get("width") == 1080 and g.get("height") == 1350
                      and abs(g.get("duration", 0) - 9) <= 0.25 and abs(g.get("fps", 0) - 30) < 0.5)


# 3. Hand-off files.
def loads(name):
    try:
        return json.loads((studio / name).read_text())
    except (OSError, ValueError):
        return None


def nonempty(name):
    f = studio / name
    return f.exists() and len(f.read_text().strip()) > 40


data, timing = loads("data.json"), loads("timing.json")
g["spec"] = nonempty("spec.md")
g["story"] = nonempty("story.md")
g["data_sourced"] = data is not None and '"source"' in json.dumps(data)
g["timing"] = timing is not None
g["handoff_ok"] = all(g[k] for k in ("spec", "story", "data_sourced", "timing"))

# 4. All six labels appear in the composition's HTML.
html = " ".join(f.read_text(errors="ignore") for f in studio.rglob("*.html") if "node_modules" not in f.parts)
g["labels"] = sum(bool(re.search(rf"\b{l}\b", html)) for l in LABELS)

g["pass"] = bool(g["check_exit"] == 0 and g["render_ok"] and g["handoff_ok"] and g["labels"] == 6)
out.write_text(json.dumps(g, indent=1))
print(json.dumps(g))
