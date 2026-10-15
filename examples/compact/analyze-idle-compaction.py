#!/usr/bin/env python3
"""Would compacting an idle conversation at 55 minutes pay off?

Reads your local transcripts (~/.claude/projects, or the directory given as
the first argument) and prints aggregates only; nothing is sent anywhere.
Opus 5.5 rates. For each main-conversation idle period of 55 minutes or more
with a context of at least the threshold, compare doing nothing with
compacting at 55 minutes while the cache is still warm:

- compaction: the context read from the cache, plus a 5K-token summary
- after it: the return writes a BASE-token context instead of the full one,
  and every later request until the next idle period or shrink re-reads
  BASE instead of the full context
- a period that never returned pays the compaction for nothing
"""
import datetime
import glob
import json
import os
import sys

INPUT, OUTPUT, READ, WRITE = 4.0, 20.0, 0.20, 8.0
IDLE = 3300
TTL = 3600
BASE = 60_000
SUMMARY = 5_000


def sessions(root):
    for path in glob.glob(os.path.join(root, "*", "*.jsonl")):
        seen, reqs = set(), []
        for line in open(path, errors="ignore"):
            try:
                e = json.loads(line)
            except ValueError:
                continue
            if e.get("type") != "assistant" or e.get("isSidechain"):
                continue
            m = e.get("message") or {}
            u = m.get("usage")
            if not u or m.get("id") in seen or "opus" not in (m.get("model") or ""):
                continue
            seen.add(m.get("id"))
            try:
                t = datetime.datetime.fromisoformat(e["timestamp"].replace("Z", "+00:00")).timestamp()
            except (KeyError, ValueError):
                continue
            ctx = (u.get("input_tokens") or 0) + (u.get("cache_read_input_tokens") or 0) \
                + (u.get("cache_creation_input_tokens") or 0)
            reqs.append((t, ctx))
        if reqs:
            yield reqs


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/.claude/projects")
    data = list(sessions(root))
    for floor in (100_000, 150_000, 250_000, 400_000):
        n = returned = 0
        compaction = rewrite_saved = reads_saved = 0.0
        for reqs in data:
            for k in range(len(reqs)):
                last = reqs[k][1]
                gap = reqs[k + 1][0] - reqs[k][0] if k + 1 < len(reqs) else None
                if last < floor or (gap is not None and gap < IDLE):
                    continue
                n += 1
                compaction += (last * READ + SUMMARY * OUTPUT + BASE * WRITE * 0) / 1e6
                if gap is None:
                    continue
                returned += 1
                ret = reqs[k + 1][1]
                # Without: a return after the TTL re-writes the whole context.
                # With: it writes BASE (the compacted context) either way.
                without_write = ret * WRITE if gap >= TTL else ret * READ
                rewrite_saved += (without_write - BASE * WRITE) / 1e6
                later = 0
                for j in range(k + 2, len(reqs)):
                    if reqs[j][1] < 0.8 * reqs[j - 1][1] or reqs[j][0] - reqs[j - 1][0] >= IDLE:
                        break
                    later += 1
                reads_saved += max(ret - BASE, 0) * later * READ / 1e6
        net = rewrite_saved + reads_saved - compaction
        print(f"context ≥ {floor // 1000}K: {n} idle periods ({returned} returned) · "
              f"compactions ${compaction:,.0f} · return writes saved ${rewrite_saved:,.0f} · "
              f"later reads saved ${reads_saved:,.0f} · net ${net:,.0f}")


if __name__ == "__main__":
    main()
