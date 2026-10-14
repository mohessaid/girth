# Performance

These are release-build timings on the author's machine (x86_64 Linux, 2026-10-14), measured as **whole process runs** (spawn + work + exit) with the median of 30.

**Ledger:** 10,000 generated events (80% use, 15% craft, 5% feedback), 2.0 MB.

| Command | Median | Max | Budget (spec §2) |
|---|---|---|---|
| `girth record use` (hook stdin) | 1.5 ms | 4.5 ms | < 10 ms ✅ |
| `girth stats` | 33.9 ms | 51.6 ms | < 50 ms ✅ (median) |
| `girth stats --json` | 36.7 ms | 67.7 ms | — |
| `girth share preview` | 44.4 ms | 77.9 ms | — |
| `girth search` (47 skills, 3.8 MB of markdown) | ~140 ms | — | — |

`record use` is a single `O_APPEND` write and doesn't read the ledger, so it stays constant as the ledger grows. `stats` and `preview` parse every line, so they scale linearly. At ~10K events a year of heavy use stays under 50 ms. Compaction can come later if needed.

## Taste hook (v0.3)

`girth record use` now also checks taste. It reads both taste tiers, but parses only the lines that mention the used skill or its domains. Measured on 2026-10-14 as whole process runs (median of 100) while the machine was loaded (load average ~18), so absolute numbers are inflated; the difference is what matters.

| Hook input | No taste file | 1,000-event taste file |
|---|---|---|
| `girth-domain-3` (has taste; injects context) | 2.35 ms | 2.85 ms |
| `unrelated` (no taste; prints nothing) | 2.31 ms | 3.33 ms |

Without the pre-filter, parsing all 1,000 lines cost ~4–6 ms in-process. Resolving, building the view and rendering cost 0.02 ms.
