"""Summarise SkillsBench proof runs: pass rate and tokens per arm."""
import glob, json, os, sys

out = sys.argv[1]
rows = {}
for path in glob.glob(os.path.join(out, "*", "*", "*", "summary.json")):
    arm, task = path.split(os.sep)[-4], path.split(os.sep)[-3]
    s = json.load(open(path))
    rows.setdefault(arm, []).append((task, s.get("passed", 0), s.get("total", 0), s.get("total_input_tokens", 0), s.get("total_output_tokens", 0), s.get("avg_skill_invocations", 0)))
print("| arm | tasks passed | input tokens | output tokens | skill invocations / task |")
print("|---|---|---|---|---|")
for arm in ("none", "curated", "girth"):
    r = rows.get(arm, [])
    if not r:
        continue
    print(f"| {arm} | {sum(x[1] for x in r)}/{sum(x[2] for x in r)} | {sum(x[3] for x in r):,} | {sum(x[4] for x in r):,} | {sum(x[5] for x in r)/len(r):.1f} |")
print()
print("| task | " + " | ".join(a for a in ("none", "curated", "girth") if a in rows) + " |")
print("|---|" + "---|" * len([a for a in ("none", "curated", "girth") if a in rows]))
tasks = sorted({x[0] for r in rows.values() for x in r})
for t in tasks:
    cells = []
    for arm in ("none", "curated", "girth"):
        if arm in rows:
            hit = [x for x in rows[arm] if x[0] == t]
            cells.append(("pass" if hit[0][1] else "fail") if hit else "—")
    print(f"| {t} | " + " | ".join(cells) + " |")
