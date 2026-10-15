#!/usr/bin/env python3
"""Where does subagent spend go in your Claude Code sessions?

Reads your local transcripts (~/.claude/projects, or the directory given as
the first argument): main conversations (<project>/<session>.jsonl) and their
subagents (<project>/<session>/subagents/agent-*.jsonl plus .meta.json).
Prints aggregates only, at API prices; nothing is sent anywhere.

Cache writes are priced by their real TTL: the 5-minute cache at 1.25x input,
the 1-hour cache at 2x input.
"""
import glob
import json
import os
import sys
from collections import defaultdict

# $/Mtok: input, output, cache read
RATES = {
    "opus": (4.0, 20.0, 0.20),
    "sonnet": (2.0, 10.0, 0.20),
    "haiku": (0.10, 0.50, 0.01),
    "fable": (10.0, 50.0, 0.25),
}


def family(model):
    for key in RATES:
        if model and key in model:
            return key
    return None


def usage_rows(path):
    """(model, input, output, read, write_5m, write_1h) per API request."""
    seen = set()
    for line in open(path, errors="ignore"):
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e.get("type") != "assistant":
            continue
        m = e.get("message") or {}
        u = m.get("usage")
        if not u or m.get("id") in seen:
            continue
        seen.add(m.get("id"))
        split = u.get("cache_creation") or {}
        w5 = split.get("ephemeral_5m_input_tokens")
        w1 = split.get("ephemeral_1h_input_tokens")
        total_w = u.get("cache_creation_input_tokens") or 0
        if w5 is None and w1 is None:
            w5, w1 = 0, total_w
        yield (m.get("model"), u.get("input_tokens") or 0, u.get("output_tokens") or 0,
               u.get("cache_read_input_tokens") or 0, w5 or 0, w1 or 0)


def cost(fam, i, o, r, w5, w1):
    inp, out, read = RATES[fam]
    return (i * inp + o * out + r * read + w5 * inp * 1.25 + w1 * inp * 2) / 1e6


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/.claude/projects")
    totals = defaultdict(lambda: [0, 0.0])          # where -> [requests, $]
    by_model = defaultdict(lambda: [0, 0.0])        # (where, family) -> [requests, $]
    by_type = defaultdict(lambda: [0, 0, 0.0, 0])   # agentType -> [agents, requests, $, output]
    writes = defaultdict(lambda: [0, 0])            # where -> [5m, 1h]
    reprice = defaultdict(float)                    # family -> subagent $ if re-run on it
    for path in glob.glob(os.path.join(root, "*", "*.jsonl")):
        for row in usage_rows(path):
            fam = family(row[0])
            if not fam:
                continue
            c = cost(fam, *row[1:])
            totals["main"][0] += 1
            totals["main"][1] += c
            by_model[("main", fam)][0] += 1
            by_model[("main", fam)][1] += c
            writes["main"][0] += row[4]
            writes["main"][1] += row[5]
    for path in glob.glob(os.path.join(root, "*", "*", "subagents", "agent-*.jsonl")):
        try:
            meta = json.load(open(path[: -len(".jsonl")] + ".meta.json"))
        except (OSError, ValueError):
            meta = {}
        kind = meta.get("agentType") or "unknown"
        by_type[kind][0] += 1
        for row in usage_rows(path):
            fam = family(row[0])
            if not fam:
                continue
            c = cost(fam, *row[1:])
            totals["subagent"][0] += 1
            totals["subagent"][1] += c
            by_model[("subagent", fam)][0] += 1
            by_model[("subagent", fam)][1] += c
            by_type[kind][1] += 1
            by_type[kind][2] += c
            by_type[kind][3] += row[2]
            writes["subagent"][0] += row[4]
            writes["subagent"][1] += row[5]
            for other in ("opus", "sonnet", "haiku"):
                reprice[other] += cost(other, *row[1:])

    spend = sum(v[1] for v in totals.values()) or 1
    for where, (n, c) in totals.items():
        w5, w1 = writes[where]
        print(f"{where:>8}: {n:6,} requests · ${c:9,.2f} ({c / spend:.1%}) · "
              f"cache writes 5m {w5:,} / 1h {w1:,}")
    print("by model:")
    for (where, fam), (n, c) in sorted(by_model.items(), key=lambda kv: -kv[1][1]):
        print(f"  {where:>8} {fam:<6} {n:6,} requests · ${c:9,.2f}")
    print("subagents by type:")
    for kind, (agents, n, c, out) in sorted(by_type.items(), key=lambda kv: -kv[1][2]):
        print(f"  {kind[:28]:<28} {agents:4} agents · {n:6,} requests · ${c:8,.2f} · output {out:,}")
    print("subagent spend if every request had run on one model (same tokens):")
    for fam, c in reprice.items():
        print(f"  {fam:<6} ${c:9,.2f}")


if __name__ == "__main__":
    main()
