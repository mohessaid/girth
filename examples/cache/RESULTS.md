# Cache keeper and doctor: results (2026-10-15)

## 1. Where cache writes come from (`analyze-breaks.py`, the author's transcripts)
108 sessions, 30,631 main-conversation requests, 147.3M cache-write tokens:

| Break after | Breaks | Re-written | Share of all writes |
|---|---|---|---|
| < 5 min idle (the prompt changed) | 11 | 3.4M | 2.3% |
| 5–60 min idle | 2 | 0.4M | 0.2% |
| > 60 min idle (the cache expired) | 208 | 92.1M | **62.5%** |

- **Claude Code writes the 1-hour cache.** Results report `ephemeral_1h_input_tokens`, and almost nothing breaks between 5 and 60 minutes.
- **Return gaps** (264 returns after 55+ minutes idle): p25 1.7 h, median 3.4 h, p75 11.1 h. Another 110 idle periods never returned.
- **Keep-warm simulation,** one ping every 55 minutes and at most N pings, counting every never-returned period as N wasted pings:

  | N | pings | rewrites avoided | net |
  |---|---|---|---|
  | 1 | $24.60 | $240.11 | +$216 |
  | 3 | $55.79 | $471.55 | **+$416** |
  | 8 | $107.39 | $616.30 | +$509 |

  These are API prices, with the 1-hour write rate. Recomputing logged runs at that rate reproduces Claude Code's `total_cost_usd`.

## 2. Loop breaker: dropped (`analyze-loops.py`)
In 32,787 tool calls (1,005 errors), the same tool with the same input returning the same error reached a third attempt **2 times**. A loop breaker would almost never fire on this usage, so it wasn't built.

## 3. End-to-end: the real mod, isolated (`GIRTH_KEEPER_INTERVAL_MS=60000`, Haiku 5.5)
The script sent turn 1, then turn 2 37 s later, then waited 80 s.
- **Exactly one ping, 62 s after turn 2.** Turn 1's pending ping was cancelled when turn 2 started. This is the wiring `claude plugin test` can't observe.
- **The ping read 21,758 of the 23,521 cached tokens (92.5%):** the fork reads the conversation's cache.
- **The doctor caught a real break.** MCP servers finished connecting between the turns (the ledger's `seen` event lists them), which changed the prompt. Turn 2 re-wrote 7,037 tokens. That is below the 20,000 line threshold, so the event was recorded with no line under the answer.
- **`girth stats --meter`:** `cache re-written 1 times: 7,037 tokens` and `keeper: 1 pings`.

## 4. Probe: does a ping really keep the cache past the hour? (`probe/`, Haiku 5.5)
- **Setup:** two sessions, each with a 72.5K-token context of its own (Girth's source behind a per-arm tag, so the arms share no cache beyond the system prompt).
- **Turns:** each arm's first turn wrote about 72.5K tokens to the 1-hour cache. Both arms resumed after 66 minutes.

| Arm | Ping at 55 min | Resume: read from cache | Resume: written again | Cost of the return |
|---|---|---|---|---|
| control | none | 11,093 (system prompt only) | **74,319** | $0.0150 |
| warm | read 83,641, wrote 67 | **83,641** | 1,840 | $0.0021 + ping $0.0009 = **$0.0029** |

- **One ping kept the whole conversation cached past the hour,** and the return cost 81% less.
- **At Opus 5.5 rates** the ratio is the same: a ping costs `context × $0.20`, and a rewrite costs `context × $8` per Mtok.
- **Note:** the first probe used random-letter filler, and Claude's safety classifier refused it. Use natural text in probes.
