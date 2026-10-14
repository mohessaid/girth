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
