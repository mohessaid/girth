"""Summarise SkillsBench proof runs: pass rate and tokens per arm."""
import glob, json, os, sys

out = sys.argv[1]
rows = {}
for path in glob.glob(os.path.join(out, "*", "*", "*", "summary.json")):
    arm, task = path.split(os.sep)[-4], path.split(os.sep)[-3]
    s = json.load(open(path))
    # BenchFlow's own counter misses Skill calls made through the ACP agent;
    # count them from the trajectory instead.
    skills = []
    for traj in glob.glob(os.path.join(os.path.dirname(path), "*", "trajectory", "acp_trajectory.jsonl")):
        for line in open(traj):
            try:
                ev = json.loads(line)
            except ValueError:
                continue
            if ev.get("type") == "tool_call" and ev.get("title") == "Skill":
                text = json.dumps(ev.get("content", ""))
                skills.append(text.split("Launching skill: ")[-1].split("\\")[0].split('"')[0] if "Launching skill" in text else "?")
    rows.setdefault(arm, []).append((task, s.get("passed", 0), s.get("total", 0), s.get("total_input_tokens", 0), s.get("total_output_tokens", 0), len(skills), skills))
print("| arm | tasks passed | input tokens | output tokens | skill calls / task |")
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
            cells.append((("pass" if hit[0][1] else "fail") + (f" ({', '.join(hit[0][6])})" if hit[0][6] else "")) if hit else "—")
    print(f"| {t} | " + " | ".join(cells) + " |")
