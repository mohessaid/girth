"""Exact chance that `girth trial` calls a task won or lost when the skill does nothing.

Both arms pass each run with the same probability p. A task is called when Fisher's
one-sided exact test puts the gap at or under 0.05 in either direction (src/trial.rs).
For each run count, print the worst case over p = 0.01 … 0.99. No simulation: exact sums.

    python3 examples/trial/false_calls.py
"""
import math


def tail(a, b, n):
    k = a + b
    hits = sum(math.comb(n, x) * math.comb(n, k - x) for x in range(a, min(n, k) + 1) if k - x <= n)
    return hits / math.comb(2 * n, k)


for n in range(2, 51):
    called = [[tail(a, b, n) <= 0.05 or tail(b, a, n) <= 0.05 for b in range(n + 1)] for a in range(n + 1)]
    worst = 0.0
    for i in range(1, 100):
        p = i / 100
        pmf = [math.comb(n, k) * p**k * (1 - p) ** (n - k) for k in range(n + 1)]
        worst = max(worst, sum(pmf[a] * pmf[b] for a in range(n + 1) for b in range(n + 1) if called[a][b]))
    print(f"{n:2d} runs: worst-case false-call rate {worst:.4f}{'  > 0.05' if worst > 0.05 else ''}")
