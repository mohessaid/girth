#!/usr/bin/env python3
"""Where do prompt-cache writes come from in your Claude Code sessions?

Reads your local transcripts (~/.claude/projects/*/*.jsonl, or the directory
given as the first argument) and prints aggregates only: cache-write tokens
that followed an idle gap, prefix changes, and a keep-warm simulation.
Nothing is sent anywhere.

A healthy request reads back what the previous one wrote
(read == prev.read + prev.write). A shortfall of >= 5,000 tokens and >= 20%
is a break, unless the prompt shrank (compaction, /clear).
"""
import datetime
import glob
import json
import os
import sys

# $/Mtok: (cache read, 1-hour cache write)
RATES = {"opus": (0.20, 8.0), "sonnet": (0.20, 4.0), "haiku": (0.01, 0.20), "fable": (0.25, 20.0)}
TTL = 3600
PING = 3300


def rates(model):
    for key, value in RATES.items():
        if model and key in model:
            return value
    return RATES["opus"]


def requests(path):
    """Main-conversation requests in one transcript: (time, model, input, read, write)."""
    seen = set()
    for line in open(path, errors="ignore"):
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e.get("type") != "assistant" or e.get("isSidechain"):
            continue
        m = e.get("message") or {}
        u = m.get("usage")
        if not u or m.get("id") in seen:
            continue
        seen.add(m.get("id"))
        try:
            t = datetime.datetime.fromisoformat(e["timestamp"].replace("Z", "+00:00")).timestamp()
        except (KeyError, ValueError):
            continue
        yield (t, m.get("model"), u.get("input_tokens") or 0,
               u.get("cache_read_input_tokens") or 0, u.get("cache_creation_input_tokens") or 0)


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/.claude/projects")
    files = glob.glob(os.path.join(root, "*", "*.jsonl"))
    total = written = shrinks = 0
    buckets = {"<5m": [0, 0], "5-60m": [0, 0], ">60m": [0, 0]}
    idle = []  # (gap seconds or None if never returned, context, model)
    for path in files:
        prev = {}
        last = None
        for t, model, i, r, w in requests(path):
            total += 1
            written += w
            p = prev.get(model)
            if p:
                expected = p[1] + p[2]
                lost = expected - r
                if lost >= 5000 and lost >= 0.2 * expected:
                    if i + r + w < 0.8 * expected:
                        shrinks += 1
                    else:
                        gap = t - p[0]
                        key = "<5m" if gap < 300 else "5-60m" if gap < TTL else ">60m"
                        buckets[key][0] += 1
                        buckets[key][1] += min(lost, w)
            prev[model] = (t, r, w)
            if last and t - last[0] >= PING:
                idle.append((t - last[0], last[1], last[2]))
            last = (t, i + r + w, model)
        if last:
            idle.append((None, last[1], last[2]))

    print(f"transcripts {len(files)} · requests {total} · cache-write tokens {written:,} · shrinks {shrinks}")
    for key, (n, tokens) in buckets.items():
        share = tokens / written if written else 0
        print(f"breaks after {key:>5} idle: {n:4d} · re-written {tokens:,} ({share:.1%} of all writes)")
    returned = sorted(g / 3600 for g, _, _ in idle if g)
    if returned:
        q = lambda f: returned[int(f * (len(returned) - 1))]
        print(f"idle periods >= 55m: {len(idle)} · returned {len(returned)} · "
              f"return gap hours p25 {q(.25):.1f} median {q(.5):.1f} p75 {q(.75):.1f}")
    print("keep-warm simulation (a ping every 55 min, at most N; unreturned periods pay N pings):")
    for n in (1, 2, 3, 4, 6, 8):
        cost = saved = 0.0
        for gap, ctx, model in idle:
            read, write = rates(model)
            pings = n if gap is None else min(n, int(gap // PING))
            cost += pings * ctx * read / 1e6
            if gap is not None and TTL <= gap < n * PING + TTL:
                saved += ctx * (write - read) / 1e6
        print(f"  N={n}: pings ${cost:,.2f} · rewrites avoided ${saved:,.2f} · net ${saved - cost:,.2f}")


if __name__ == "__main__":
    main()
