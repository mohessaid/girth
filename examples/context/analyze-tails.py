#!/usr/bin/env python3
"""How much of the tool traffic you re-read could a trim reach?

Reads your local transcripts (~/.claude/projects, or the directory given as
the first argument) and prints aggregates only; nothing is sent anywhere.

For each tool call's input and result it records the size and the number of
later main requests that carry it (until compaction or /clear), then prints
each kind's share of the re-read tool tokens, its size percentiles, and the
part that comes from blocks over 2K tokens: what a trim of large results
could touch. Bash commands that carry a heredoc are split out, because they
are file writes. Sizes are estimated at 4 characters per token.
"""
import glob
import json
import os
import sys
from collections import defaultdict

CHARS_PER_TOKEN = 4
LARGE = 2000  # tokens


def text_size(content):
    if isinstance(content, str):
        return len(content)
    if isinstance(content, list):
        return sum(text_size(c.get("text") or c.get("content") or "") if isinstance(c, dict) else 0
                   for c in content)
    return 0


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/.claude/projects")
    blocks = []  # [kind, tokens, requests that carried it]
    for path in glob.glob(os.path.join(root, "*", "*.jsonl")):
        live = []
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
                        name = b.get("name") or "?"
                        names[b.get("id")] = name
                        inp = b.get("input") or {}
                        kind = "input: " + name
                        if name == "Bash" and "<<" in (inp.get("command") or ""):
                            kind = "input: Bash heredoc"
                        block = [kind, len(json.dumps(inp)) / CHARS_PER_TOKEN, 0]
                        live.append(block)
                        blocks.append(block)
                u = m.get("usage")
                if u and m.get("id") not in seen:
                    seen.add(m.get("id"))
                    ctx = (u.get("input_tokens") or 0) + (u.get("cache_read_input_tokens") or 0) \
                        + (u.get("cache_creation_input_tokens") or 0)
                    if prev_ctx and ctx < 0.8 * prev_ctx:
                        live = []  # compaction or /clear
                    prev_ctx = ctx
                    for block in live:
                        block[2] += 1
            elif e.get("type") == "user" and isinstance(content, list):
                for b in content:
                    if isinstance(b, dict) and b.get("type") == "tool_result":
                        block = ["result: " + names.get(b.get("tool_use_id"), "?"),
                                 text_size(b.get("content")) / CHARS_PER_TOKEN, 0]
                        live.append(block)
                        blocks.append(block)

    total = sum(t * k for _, t, k in blocks) or 1
    by_kind = defaultdict(list)
    for kind, tokens, carried in blocks:
        by_kind[kind].append((tokens, carried))
    large_results = sum(t * k for kind, t, k in blocks if kind.startswith("result: ") and t > LARGE)
    print(f"tool tokens re-read {total / 1e9:.2f}B · results over {LARGE:,} tokens: {large_results / total:.1%}")
    print(f"{'kind':<34} {'blocks':>7} {'share':>6} {'p50':>6} {'p95':>6} {'max':>7} {'over 2K':>8} {'carried':>8}")
    for kind, v in sorted(by_kind.items(), key=lambda kv: -sum(t * k for t, k in kv[1]))[:12]:
        cost = sum(t * k for t, k in v)
        sizes = sorted(t for t, _ in v)
        pct = lambda q: sizes[int(q * (len(sizes) - 1))]
        large = sum(t * k for t, k in v if t > LARGE)
        print(f"{kind[:34]:<34} {len(v):7,} {cost / total:6.1%} {pct(.5):6.0f} {pct(.95):6.0f} "
              f"{sizes[-1]:7.0f} {large / cost if cost else 0:8.1%} {sum(k for _, k in v) / len(v):8.0f}")


if __name__ == "__main__":
    main()
