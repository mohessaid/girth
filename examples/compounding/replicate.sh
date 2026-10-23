#!/usr/bin/env bash
# Usage: replicate.sh <workdir> <first> <last>
# Repeats the five-session benchmark: per repeat, bulk then jit then girth, one arm at a time
# (every session runs `hyperframes check`, so arms never overlap). Usage lands in
# replicate/r<N>/<arm>/<session>.json. Afterwards every composition the arm produced is
# checked by a script (../hyperframes/grade.py logic), not by the session's own report.
set -euo pipefail
work=$1 first=$2 last=$3
here=$(cd "$(dirname "$0")" && pwd)
for r in $(seq "$first" "$last"); do
  for arm in bulk jit girth; do
    "$here/run.sh" "$arm" "$work/r$r" > /dev/null
    out="$here/replicate/r$r/$arm"; mkdir -p "$out"
    cp "$work/r$r/$arm/usage/"*.json "$out/"
    python3 - "$work/r$r/$arm/project" "$out/grade.json" <<'PY'
import json, subprocess, sys
from pathlib import Path
project, out = Path(sys.argv[1]), Path(sys.argv[2])
def total(n, k):
    if isinstance(n, dict): return n.get(k, 0) + sum(total(v, k) for kk, v in n.items() if kk != k)
    return sum(total(v, k) for v in n) if isinstance(n, list) else 0
comps = [project / f"s{i}" for i in range(1, 6)]  # each task names its folder; a bare index.html counts
res = {}
for c in comps:
    if not (c / "index.html").exists():
        res[c.name] = {"exit": None, "error": "no index.html"}; continue
    try:
        r = subprocess.run(["npx", "-y", "hyperframes", "check", "--json"], cwd=c, capture_output=True, text=True, timeout=600)
        rep, _ = json.JSONDecoder().raw_decode(r.stdout[r.stdout.index("{"):])
        res[c.name] = {"exit": r.returncode, "errors": total(rep, "errorCount"), "warnings": total(rep, "warningCount")}
    except Exception as e:
        res[c.name] = {"exit": None, "error": str(e)[:200]}
g = {"compositions": len(comps), "passed": sum(v.get("exit") == 0 for v in res.values()), "checks": res}
out.write_text(json.dumps(g, indent=1)); print(sys.argv[1].split("/")[-2], g["compositions"], g["passed"])
PY
    echo "r$r $arm: \$$(python3 -c "import json,glob;print(round(sum(json.load(open(f))['total_cost_usd'] for f in glob.glob('$out/[1-5].json')),3))")"
  done
done
