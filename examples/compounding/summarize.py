"""Print one line per session, or with --table, a per-arm summary of usage JSON files."""
import json
import sys
from pathlib import Path


def usage(path):
    d = json.loads(Path(path).read_text())
    u = d.get("usage", {})
    return {
        "cost": d.get("total_cost_usd", 0.0),
        "turns": d.get("num_turns", 0),
        "fresh": u.get("input_tokens", 0) + u.get("cache_creation_input_tokens", 0),
        "cached": u.get("cache_read_input_tokens", 0),
        "out": u.get("output_tokens", 0),
    }


def line(label, r):
    return (f"{label}: cost ${r['cost']:.4f}, fresh input {r['fresh']:,} tok, "
            f"cache reads {r['cached']:,} tok, output {r['out']:,} tok, turns {r['turns']}")


if sys.argv[1] == "--table":
    work = Path(sys.argv[2])
    print("| Arm | s1 | s2 | s3 | s4 | s5 | Total cost | Fresh input tok | Cache-read tok |")
    print("|---|---|---|---|---|---|---|---|---|")
    for arm in ("bulk", "jit", "girth"):
        rows = [usage(work / arm / "usage" / f"{n}.json") for n in range(1, 6)
                if (work / arm / "usage" / f"{n}.json").exists()]
        if not rows:
            continue
        per = " | ".join(f"${r['cost']:.3f}" for r in rows)
        print(f"| {arm} | {per} | **${sum(r['cost'] for r in rows):.3f}** | "
              f"{sum(r['fresh'] for r in rows):,} | {sum(r['cached'] for r in rows):,} |")
else:
    print(line(sys.argv[2], usage(sys.argv[1])))
