"""Summarise runs/<arm>-<n>.json (usage) and runs/<arm>-<n>.grade.json (mechanical grade).

  summarize.py --line  <runs-dir> <arm-n>   one run
  summarize.py --spent <runs-dir> <arm>     total cost so far, for the budget stop
  summarize.py --table <runs-dir>           per arm: mean ± 95% CI (t), and the pass rate
"""
import json
import statistics
import sys
from pathlib import Path

# two-sided 95% t critical values by degrees of freedom
T95 = {1: 12.71, 2: 4.30, 3: 3.18, 4: 2.78, 5: 2.57, 6: 2.45, 7: 2.36, 8: 2.31, 9: 2.26, 10: 2.23}


def usage(path):
    try:
        d = json.loads(Path(path).read_text())
    except (OSError, ValueError):
        return None
    # modelUsage is the whole session; "usage" and num_turns cover only the last result, which is
    # a short follow-up turn whenever a background task finishes after the main answer.
    mu = d.get("modelUsage", {}).values()
    return {"cost": d.get("total_cost_usd", 0.0),
            "fresh": sum(m.get("inputTokens", 0) + m.get("cacheCreationInputTokens", 0) for m in mu),
            "cached": sum(m.get("cacheReadInputTokens", 0) for m in mu),
            "out": sum(m.get("outputTokens", 0) for m in mu)}


def grade(path):
    try:
        return json.loads(Path(path).read_text())
    except (OSError, ValueError):
        return {}


def ci(xs):
    m = statistics.mean(xs)
    if len(xs) < 2:
        return m, float("nan")
    return m, T95.get(len(xs) - 1, 1.96) * statistics.stdev(xs) / len(xs) ** 0.5


def runs(d, arm):
    names = sorted(p.name[:-5] for p in Path(d).glob(f"{arm}-*.json") if not p.name.endswith(".grade.json"))
    return [(n, usage(Path(d) / f"{n}.json"), grade(Path(d) / f"{n}.grade.json")) for n in names]


mode, d = sys.argv[1], sys.argv[2]
if mode == "--spent":
    print(f"{sum(u['cost'] for _, u, _ in runs(d, sys.argv[3]) if u):.4f}")
elif mode == "--line":
    n = sys.argv[3]
    u, g = usage(Path(d) / f"{n}.json") or {}, grade(Path(d) / f"{n}.grade.json")
    print(f"{n}: pass={g.get('pass')} tsc_exit={g.get("tsc_exit")} render_ok={g.get('render_ok')} "
          f"handoff_ok={g.get('handoff_ok')} labels={g.get('labels')}/6 | cost ${u.get('cost', 0):.3f} "
          f"fresh {u.get('fresh', 0):,} cached {u.get('cached', 0):,} out {u.get('out', 0):,}")
else:
    print("| Arm | Runs | Pass | Cost per run | Fresh input tok | Cache-read tok | Output tok |")
    print("|---|---|---|---|---|---|---|")
    for arm in ("bulk", "girth"):
        rs = [(u, g) for _, u, g in runs(d, arm) if u]
        if not rs:
            continue

        def cell(k, fmt):
            m, h = ci([u[k] for u, _ in rs])
            return f"{fmt(m)} ± {fmt(h)}"

        passed = sum(bool(g.get("pass")) for _, g in rs)
        print(f"| {arm} | {len(rs)} | {passed}/{len(rs)} | {cell('cost', lambda x: f'${x:.3f}')} | "
              f"{cell('fresh', lambda x: f'{x:,.0f}')} | {cell('cached', lambda x: f'{x:,.0f}')} | "
              f"{cell('out', lambda x: f'{x:,.0f}')} |")
