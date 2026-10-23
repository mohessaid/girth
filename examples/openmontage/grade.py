"""Mechanical grade of one run's ./video/ Remotion project, so no model judges its own output.

Usage: grade.py <project-dir> <out.json>
Pass = `tsc --noEmit` exits 0, the render matches the brief, every hand-off file is valid, and all
six panel labels are in the source (or in a hand-off JSON it imports).
"""
import json
import re
import subprocess
import sys
from pathlib import Path

project, out = Path(sys.argv[1]), Path(sys.argv[2])
video = project / "video"
LABELS = ["Reference", "Data", "Story", "Motion", "Direct", "Ship"]
g = {}


def run(cmd, cwd):
    try:
        return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=600)
    except (OSError, subprocess.TimeoutExpired) as e:
        return subprocess.CompletedProcess(cmd, 99, "", str(e))


# 1. The source type-checks (the project's own tsconfig, strict).
src = [f for f in (video / "src").rglob("*") if f.suffix in (".ts", ".tsx")] if (video / "src").exists() else []
g["src_files"] = len(src)
g["tsc_exit"] = run(["npx", "tsc", "--noEmit"], video).returncode if src else None

# 2. The render: exists, portrait 1080x1350, 9 s (±0.25), 30 fps.
mp4 = video / "final.mp4"
g["render"] = mp4.exists() and mp4.stat().st_size > 0
if g["render"]:
    p = run(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
             "stream=width,height,r_frame_rate:format=duration", "-of", "json", str(mp4)], video)
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
        return json.loads((video / name).read_text())
    except (OSError, ValueError):
        return None


def nonempty(name):
    f = video / name
    return f.exists() and len(f.read_text().strip()) > 40


data, timing = loads("data.json"), loads("timing.json")
g["spec"] = nonempty("spec.md")
g["story"] = nonempty("story.md")
g["data_sourced"] = data is not None and '"source"' in json.dumps(data)
g["timing"] = timing is not None
g["handoff_ok"] = all(g[k] for k in ("spec", "story", "data_sourced", "timing"))

# 4. All six labels appear in the source, or in a hand-off JSON the source imports
#    (both pilot runs kept them in timing.json, which the brief ties to the composition).
imported = [video / n for n in ("timing.json", "data.json")
            if (video / n).exists() and any(n in f.read_text(errors="ignore") for f in src)]
code = " ".join(f.read_text(errors="ignore") for f in src + imported)
g["labels"] = sum(bool(re.search(rf"\b{l}\b", code)) for l in LABELS)

g["pass"] = bool(g["tsc_exit"] == 0 and g["render_ok"] and g["handoff_ok"] and g["labels"] == 6)
out.write_text(json.dumps(g, indent=1))
print(json.dumps(g))
