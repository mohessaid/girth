#!/usr/bin/env python3
"""How often does an agent repeat an identical failing tool call?

Reads your local transcripts (~/.claude/projects/*/*.jsonl, or the directory
given as the first argument) and prints aggregates only. A loop is the same
tool with the same input returning the same error 3 times, with no success
of that call in between. Nothing is sent anywhere.
"""
import glob
import hashlib
import json
import os
import sys
from collections import Counter, defaultdict


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/.claude/projects")
    calls = errors = 0
    loops, repeats = Counter(), Counter()
    for path in glob.glob(os.path.join(root, "*", "*.jsonl")):
        uses, counts = {}, defaultdict(int)
        for line in open(path, errors="ignore"):
            try:
                content = (json.loads(line).get("message") or {}).get("content")
            except ValueError:
                continue
            if not isinstance(content, list):
                continue
            for block in content:
                if not isinstance(block, dict):
                    continue
                if block.get("type") == "tool_use":
                    uses[block["id"]] = (block.get("name"), digest(block.get("input")))
                    calls += 1
                elif block.get("type") == "tool_result" and block.get("tool_use_id") in uses:
                    tool, inp = uses[block["tool_use_id"]]
                    if not block.get("is_error"):
                        for key in [k for k in counts if k[:2] == (tool, inp)]:
                            del counts[key]
                        continue
                    errors += 1
                    key = (tool, inp, digest(block.get("content")))
                    counts[key] += 1
                    if counts[key] >= 2:
                        repeats[tool] += 1
                    if counts[key] == 3:
                        loops[tool] += 1
    print(f"tool calls {calls:,} · errors {errors:,}")
    print(f"identical failing call reaching 3 attempts: {sum(loops.values())} {dict(loops.most_common(5))}")
    print(f"identical failing repeats (2nd attempt on): {sum(repeats.values())}")


if __name__ == "__main__":
    main()
