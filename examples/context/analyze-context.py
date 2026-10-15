#!/usr/bin/env python3
"""What fills the context you pay to re-read on every request?

Reads your local transcripts (~/.claude/projects, or the directory given as
the first argument) and prints aggregates only; nothing is sent anywhere.

Every request re-reads the whole conversation from the cache, so a block's
true cost is its size times the number of later requests that carry it,
until the context shrinks (compaction or /clear). Sizes are estimated at
4 characters per token and charged at the Opus cache-read rate.
"""
import glob
import json
import os
import sys
from collections import defaultdict

READ = 0.20  # $/Mtok, Opus cache read
CHARS_PER_TOKEN = 4


def text_size(content):
    if isinstance(content, str):
        return len(content)
    if isinstance(content, list):
        return sum(text_size(c.get("text") or c.get("content") or "") if isinstance(c, dict) else 0
                   for c in content)
    return 0


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/.claude/projects")
    carried = defaultdict(float)   # kind -> token-requests carried
    added = defaultdict(int)       # kind -> tokens added
    count = defaultdict(int)
    stale = defaultdict(float)     # kind -> token-requests carried after the block was 20+ requests old
    requests = 0
    for path in glob.glob(os.path.join(root, "*", "*.jsonl")):
        live = []  # [kind, tokens, age in requests]
        prev_ctx = 0
        seen = set()
        names = {}
        for line in open(path, errors="ignore"):
            try:
                e = json.loads(line)
            except ValueError:
                continue
            if e.get("isSidechain"):
                continue
            m = e.get("message") or {}
            content = m.get("content")
            if e.get("type") == "assistant":
                for b in content if isinstance(content, list) else []:
                    if isinstance(b, dict) and b.get("type") == "tool_use":
                        names[b.get("id")] = b.get("name") or "?"
                        live.append(["input: " + (b.get("name") or "?"), len(json.dumps(b.get("input"))) / CHARS_PER_TOKEN, 0])
                        added["input: " + (b.get("name") or "?")] += len(json.dumps(b.get("input"))) / CHARS_PER_TOKEN
                        count["input: " + (b.get("name") or "?")] += 1
                    elif isinstance(b, dict) and b.get("type") == "thinking":
                        # The signature is not context; transcripts may also omit the text,
                        # so this undercounts thinking. Prefer the exact growth split.
                        size = len(b.get("thinking") or "") / CHARS_PER_TOKEN
                        live.append(["thinking", size, 0])
                        added["thinking"] += size
                        count["thinking"] += 1
                    elif isinstance(b, dict) and b.get("type") == "text":
                        live.append(["assistant text", len(b.get("text") or "") / CHARS_PER_TOKEN, 0])
                u = m.get("usage")
                if u and m.get("id") not in seen:
                    seen.add(m.get("id"))
                    ctx = (u.get("input_tokens") or 0) + (u.get("cache_read_input_tokens") or 0) \
                        + (u.get("cache_creation_input_tokens") or 0)
                    if prev_ctx and ctx < 0.8 * prev_ctx:
                        live = []  # compaction or /clear
                    prev_ctx = ctx
                    requests += 1
                    for block in live:
                        carried[block[0]] += block[1]
                        if block[2] >= 20:
                            stale[block[0]] += block[1]
                        block[2] += 1
            elif e.get("type") == "user":
                if isinstance(content, str):
                    live.append(["user prompt", len(content) / CHARS_PER_TOKEN, 0])
                    continue
                for b in content if isinstance(content, list) else []:
                    if not isinstance(b, dict):
                        continue
                    if b.get("type") == "tool_result":
                        kind = "result: " + names.get(b.get("tool_use_id"), "?")
                        size = text_size(b.get("content")) / CHARS_PER_TOKEN
                    elif b.get("type") == "text":
                        kind, size = "user text/attachments", len(b.get("text") or "") / CHARS_PER_TOKEN
                    else:
                        continue
                    live.append([kind, size, 0])
                    added[kind] += size
                    count[kind] += 1

    total = sum(carried.values()) or 1
    print(f"main requests {requests:,} · conversation tokens re-read {total / 1e9:.2f}B "
          f"(≈ ${total * READ / 1e6:,.0f} at the Opus read rate; system prompt and tools not included)")
    print(f"{'kind':<34} {'blocks':>7} {'avg tok':>8} {'share of re-reads':>18} {'of it 20+ requests old':>23}")
    merged = defaultdict(lambda: [0.0, 0.0, 0, 0])
    for kind, v in carried.items():
        key = kind if carried[kind] / total >= 0.01 else "other (<1% each)"
        merged[key][0] += v
        merged[key][1] += stale[kind]
        merged[key][2] += count[kind]
        merged[key][3] += added[kind]
    for kind, (v, s, n, a) in sorted(merged.items(), key=lambda kv: -kv[1][0]):
        avg = a / n if n else 0
        print(f"{kind[:34]:<34} {n:7,} {avg:8,.0f} {v / total:17.1%} {s / v if v else 0:22.0%}")


if __name__ == "__main__":
    main()
