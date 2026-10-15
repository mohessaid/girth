"""Summarise Lens A/B runs: cost, tokens, skills invoked, metadata check."""
import glob, json, os, re, sys

out = sys.argv[1]
CHECKS = ["og:title", "twitter:card", "application/ld+json", "Bakery", "openingHours", 'rel="canonical"']
rows = []
for path in sorted(glob.glob(os.path.join(out, "*.jsonl"))):
    name = os.path.basename(path)[:-6]
    skills, result = [], {}
    for line in open(path):
        try:
            ev = json.loads(line)
        except ValueError:
            continue
        if ev.get("type") == "assistant":
            for block in ev.get("message", {}).get("content", []):
                if block.get("type") == "tool_use" and block.get("name") == "Skill":
                    skills.append(block.get("input", {}).get("skill"))
        if ev.get("type") == "result":
            result = ev
    html_path = os.path.join(out, name + ".html")
    html = open(html_path).read() if os.path.exists(html_path) else ""
    passed = sum(c in html for c in CHECKS)
    usage = result.get("usage", {})
    lens = []
    ledger = os.path.join(out, name + ".girth", "ledger.jsonl")
    if os.path.exists(ledger):
        for line in open(ledger):
            ev = json.loads(line)
            if ev.get("kind") == "turn" and ev.get("lens_before"):
                lens.append((ev["lens_before"], ev["lens_after"]))
    rows.append((name, result.get("total_cost_usd", 0.0), usage.get("input_tokens", 0) + usage.get("cache_creation_input_tokens", 0), usage.get("cache_read_input_tokens", 0), passed, skills, lens[:1]))
print("| run | cost $ | input+cache write | cache read | metadata checks | skills invoked | lens bytes before→after |")
print("|---|---|---|---|---|---|---|")
for r in rows:
    print(f"| {r[0]} | {r[1]:.3f} | {r[2]:,} | {r[3]:,} | {r[4]}/{len(CHECKS)} | {', '.join(s for s in r[5] if s) or '—'} | {r[6][0][0]:,}→{r[6][0][1]:,} |" if r[6] else f"| {r[0]} | {r[1]:.3f} | {r[2]:,} | {r[3]:,} | {r[4]}/{len(CHECKS)} | {', '.join(s for s in r[5] if s) or '—'} | — |")
