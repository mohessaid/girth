#!/usr/bin/env python3
"""What does an outcome cost in your Claude Code sessions?

Reads your local transcripts (~/.claude/projects, or the directory given as
the first argument). Pairs each Bash call with its result, classifies the
command (test run, commit, pull request), and prices the model requests
between outcomes at API prices. Prints aggregates only; nothing is sent
anywhere. With --unmatched it also prints the most common command stems it
could not classify, so the classifier can be checked against real commands
(that list is for your terminal, not for a document).
"""
import glob
import json
import os
import re
import sys
from collections import Counter, defaultdict

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "subagents"))
from importlib import import_module  # noqa: E402

sub = import_module("analyze-subagents")

TEST = re.compile(
    r"^(cargo (\+\S+ )?(test|nextest|llvm-cov)|make (check|test)\b|(npm|pnpm|yarn|bun)( run)? test\b"
    r"|npx (vitest|jest|playwright test)|vitest\b|jest\b|pytest\b|python3? -m pytest|go test\b"
    r"|deno test\b|flutter test\b|dart test\b|mix test\b|rspec\b|phpunit\b|claude plugin test\b)"
)
COMMIT = re.compile(r"^git( -C \S+)* commit\b(?!.*--dry-run)")
PR = re.compile(r"^gh pr (create|merge)\b")
PREFIX = re.compile(r"^(rtk( proxy)? |sudo |time |env |timeout \S+ |nice -n \S+ |mise exec( \S+)* -- |uv run |[A-Z_][A-Z0-9_]*=\S* )+")


HEREDOC = re.compile(r"<<-?\s*(['\"]?)(\w+)\1[^\n]*\n.*?^\s*\2\s*$", re.S | re.M)


def segments(command):
    """The simple commands of a command line, normalised, heredoc bodies dropped."""
    command = HEREDOC.sub("", command)
    for part in re.split(r"&&|\|\||;|\||\n", command):
        part = part.strip().strip("()").strip()
        part = PREFIX.sub("", part)
        part = re.sub(r"^(/usr)?(/local)?/s?bin/", "", part)
        if part and not part.startswith("cd "):
            yield part


# A runner's own summary decides a test run: agents pipe test output through
# tail or grep, so the exit status is almost always the last filter's.
FAILED = re.compile(
    r"test result: FAILED|^FAIL\b|^FAILED\b|\b[1-9]\d* (failed|failures?|errors?)\b|^\W*fail [1-9]|^\s*[1-9]\d* fail\b"
    r"|error(\[E\d+\])?: could not compile|Tests?:?\s+[1-9]\d* failed|make: \*\*\* .*Error",
    re.M | re.I,
)
PASSED = re.compile(
    r"test result: ok\.|\b\d+ passed\b|^ok\s+\S|Tests?:?\s+\d+ passed|^PASS\b"
    r"|\b\d+ tests?, 0 failures\b|^\W*fail 0\b|^\s*\d+ pass\b|^Ran \d+ tests? across",
    re.M | re.I,
)


def verdict(output, is_error):
    """'fail', 'pass' or 'unknown' for a test run, from the end of its output."""
    tail = output[-4000:]
    if is_error or FAILED.search(tail):
        return "fail"
    return "pass" if PASSED.search(tail) else "unknown"


FILTERS = {"grep", "egrep", "fgrep", "rg", "tail", "head", "sed", "awk", "cut", "sort", "uniq", "wc", "tr"}


def ends_in_filter(command):
    """The last pipeline ends in a text filter, which owns the exit status."""
    last = [p for p in re.split(r"&&|\|\||;|\n", HEREDOC.sub("", command)) if p.strip()]
    if not last:
        return False
    stages = re.split(r"(?<!\|)\|(?!\|)", last[-1])
    words = list(segments(stages[-1])) if len(stages) > 1 else []
    return bool(words) and words[0].split()[0] in FILTERS


def classify(command):
    kinds = set()
    for seg in segments(command):
        if TEST.match(seg):
            kinds.add("test")
        elif COMMIT.match(seg):
            kinds.add("commit")
        elif PR.match(seg):
            kinds.add("pr")
    return kinds


def stem(command):
    for seg in segments(command):
        return " ".join(seg.split()[:2])
    return ""


def events(path):
    """(timestamp, kind, payload) in file order: 'cost' and 'outcome' rows."""
    seen = set()
    calls = {}
    for line in open(path, errors="ignore"):
        try:
            e = json.loads(line)
        except ValueError:
            continue
        ts = e.get("timestamp") or ""
        m = e.get("message") or {}
        if e.get("type") == "assistant":
            u = m.get("usage")
            fam = sub.family(m.get("model"))
            if u and fam and m.get("id") not in seen:
                seen.add(m.get("id"))
                split = u.get("cache_creation") or {}
                w5 = split.get("ephemeral_5m_input_tokens")
                w1 = split.get("ephemeral_1h_input_tokens")
                if w5 is None and w1 is None:
                    w5, w1 = 0, u.get("cache_creation_input_tokens") or 0
                yield ts, "cost", sub.cost(fam, u.get("input_tokens") or 0, u.get("output_tokens") or 0,
                                           u.get("cache_read_input_tokens") or 0, w5 or 0, w1 or 0)
            for b in m.get("content") or []:
                if isinstance(b, dict) and b.get("type") == "tool_use" and b.get("name") == "Bash":
                    calls[b.get("id")] = (b.get("input") or {}).get("command") or ""
        content = m.get("content")
        if e.get("type") == "user" and isinstance(content, list):
            for b in content:
                if not isinstance(b, dict) or b.get("type") != "tool_result":
                    continue
                command = calls.pop(b.get("tool_use_id"), None)
                if command is None:
                    continue
                r = e.get("toolUseResult")
                background = isinstance(r, dict) and (r.get("backgroundTaskId") or r.get("interrupted"))
                text = b.get("content")
                if isinstance(text, list):
                    text = " ".join(x.get("text", "") for x in text if isinstance(x, dict))
                ok = not b.get("is_error") and not background
                error = bool(b.get("is_error")) and not ends_in_filter(command)
                yield ts, "outcome", (command, ok, bool(background), verdict(text or "", error))


def pct(values, q):
    if not values:
        return 0.0
    values = sorted(values)
    return values[min(len(values) - 1, int(q * len(values)))]


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    root = args[0] if args else os.path.expanduser("~/.claude/projects")
    per = defaultdict(lambda: {"spend": 0.0, "commits": 0, "tests": 0, "green": 0, "prs": 0,
                               "segments": [], "since": 0.0, "red": 0.0, "red_now": False})
    unmatched = Counter()
    verdicts = Counter()
    background = 0
    for path in glob.glob(os.path.join(root, "*", "*.jsonl")):
        p = per[os.path.basename(os.path.dirname(path))]
        rows = list(events(path))
        sidecar = os.path.join(path[: -len(".jsonl")], "subagents")
        sub_spend = 0.0
        for agent in glob.glob(os.path.join(sidecar, "agent-*.jsonl")):
            for row in sub.usage_rows(agent):
                fam = sub.family(row[0])
                if fam:
                    sub_spend += sub.cost(fam, *row[1:])
        p["spend"] += sub_spend
        p["since"] += sub_spend
        for _, kind, payload in rows:
            if kind == "cost":
                p["spend"] += payload
                p["since"] += payload
                if p["red_now"]:
                    p["red"] += payload
                continue
            command, ok, bg, result = payload
            background += bg
            kinds = classify(command)
            if not kinds:
                unmatched[stem(command)] += 1
                continue
            if "test" in kinds and not bg:
                p["tests"] += 1
                p["green"] += result == "pass"
                verdicts[result] += 1
                if result != "unknown":
                    p["red_now"] = result == "fail"
            if "commit" in kinds and ok:
                p["commits"] += 1
                p["segments"].append(p["since"])
                p["since"] = 0.0
            if "pr" in kinds and ok:
                p["prs"] += 1

    total = defaultdict(float)
    segs = []
    print(f"{'project':<10} {'spend':>9} {'commits':>7} {'$/commit':>9} {'tests':>6} {'green':>6} {'$/green':>8} {'PRs':>4}")
    rows = sorted(per.items(), key=lambda kv: -kv[1]["spend"])
    for i, (name, p) in enumerate(rows):
        for k in ("spend", "commits", "tests", "green", "prs", "red"):
            total[k] += p[k]
        segs += p["segments"]
        if p["spend"] < 20:
            continue
        per_commit = p["spend"] / p["commits"] if p["commits"] else float("nan")
        per_green = p["spend"] / p["green"] if p["green"] else float("nan")
        print(f"P{i + 1:<9} ${p['spend']:8,.0f} {p['commits']:7} {per_commit:9.2f} {p['tests']:6} "
              f"{p['green']:6} {per_green:8.2f} {p['prs']:4}")
    print(f"{'all':<10} ${total['spend']:8,.0f} {int(total['commits']):7} "
          f"{total['spend'] / max(total['commits'], 1):9.2f} {int(total['tests']):6} {int(total['green']):6} "
          f"{total['spend'] / max(total['green'], 1):8.2f} {int(total['prs']):4}")
    print(f"spend between commits (n={len(segs)}): p25 ${pct(segs, .25):.2f} · median ${pct(segs, .5):.2f} · "
          f"p75 ${pct(segs, .75):.2f} · p95 ${pct(segs, .95):.2f}")
    print(f"spend while the last test run had failed: ${total['red']:,.0f} ({total['red'] / total['spend']:.1%})")
    print(f"backgrounded or interrupted Bash calls: {background}")
    print("test runs by verdict: " + " · ".join(f"{k} {n}" for k, n in verdicts.most_common()))
    if "--unmatched" in sys.argv:
        print("top unmatched command stems:")
        for s, n in unmatched.most_common(40):
            print(f"  {n:6} {s}")


if __name__ == "__main__":
    main()
