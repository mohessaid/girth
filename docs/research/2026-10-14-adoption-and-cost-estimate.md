# Girth: adoption and infrastructure cost estimate

**Date:** 2026-10-14. All figures were gathered that day from GitHub, npm, skills.sh and provider pricing pages (sources at the end). Treat this as an order-of-magnitude model, not a forecast.

## 1. Comparable projects

| Project | What it is | Stars | Age | Usage signal |
|---|---|---|---|---|
| JuliusBrussee/caveman | Token-saving output style (skill) | 111.7K | ~6 months | n/a |
| rtk-ai/rtk | Rust token-saving CLI proxy | 83.3K | ~9 months | n/a |
| obra/superpowers | Skills plugin (process) | 298.9K | ~12 months | n/a |
| mattpocock/skills | Skill collection | 290.6K | ~8 months | ~1M installs per top skill |
| heygen-com/hyperframes | Video skills + CLI | 63.3K | ~7 months | ~1.0M installs (cli skill) |
| vercel-labs/agent-browser | Browser skill | 44.0K | ~9 months | 1.1M installs |
| ccusage | Claude Code usage/cost CLI | 19.0K | ~16 months | 878K npm downloads/month |
| vercel-labs/skills (`npx skills`) | Skills CLI | 34.8K | ~9 months | 25.1M npm downloads/month |
| skillselion-mcp | JIT skill loading MCP (closest prior art) | 1 | ~4 months | 335 npm downloads/month |
| @anthropic-ai/claude-code | The harness | — | — | 55.1M npm downloads/month |

**Ecosystem size:** skills.sh shows ~1.32M entries in its all-time index, and the top skill (`find-skills`) has 3.8M installs.

### What the comparables say
1. **Token-saving tools with a visible number break out.** caveman (112K stars in 6 months) and rtk (83K in 9 months) are the closest analogues to Girth's pitch. Girth has a stronger, *measured* number (38× lower always-on skill cost).
2. **Discovery alone does not attract.** skillselion-mcp, which is functionally the closest prior art (JIT loading plus synthesis), has 1 star and 335 downloads a month. People adopt *savings and quality*, not catalogues.
3. **Demand for finding skills is huge.** `npx skills` gets 25M downloads a month, and find-skills has 3.8M installs. This is the audience Girth's index and ledger serve.
4. **Stars-to-usage ratios:** installs ≈ 15–25× stars (hyperframes, agent-browser). Monthly active users are far lower than installs; for ccusage, roughly 30K MAU against 19K stars, assuming each npm download is one run and ~30 runs per user per month. This model uses **MAU ≈ 1× stars** as a middle assumption.

## 2. Adoption scenarios (12 months after launch)

| Scenario | Stars | Monthly active users | Opt-in sharing (15%) | Analogue |
|---|---|---|---|---|
| Low (niche) | 2K | 2K | 300 | a typical well-made plugin |
| Base | 15K | 15K | 2.3K | ccusage-like |
| Breakout | 100K | 150K | 22.5K | caveman / rtk |

**Assumptions:**
- 40 Girth-touching sessions per active user per month.
- 100 searches per user per month, 10% of which hit the online long-tail API (the rest are answered by the local snapshot).
- Each opted-in session uploads **one aggregated batch** that writes ~5 counter rows.

### Workload per scenario

| Per month | Low | Base | Breakout |
|---|---|---|---|
| Signal uploads (batches) | 12K | 90K | 900K |
| Counter rows written | 60K | 450K | 4.5M |
| Long-tail API queries | 20K | 150K | 1.5M |
| Snapshot downloads (weekly, ~15 MB curated tier) | 8K / 120 GB | 60K / 0.9 TB | 600K / 9 TB |

### The index itself
- **Size:** ~1.3M skills × ~30 sections × ~80 B per fingerprint row ≈ 3 GB, plus ~1 GB of metadata. Deduplication collapses copies, so the canonical set is smaller.
- **What clients get:** a **curated tier** (originals plus skills with real signals, ~50K entries, ~15 MB compressed). The long tail is served online.
- **Crawling:** first crawl ~1.3M files. Daily incremental work is change detection via GitHub GraphQL (`pushedAt`, 100 repos per query), then shallow clones of changed repos only (~2% a day).

## 3. Cost by architecture

### A. Git-native index + Cloudflare (recommended)
- The crawler runs on **GitHub Actions**, which is free for public repos.
- Snapshots go to **R2**: 10 GB free, then $0.015/GB-month, **with free egress**.
- Live signals go to a **Worker + D1**:
  - Free: 100K requests/day, and D1 at 5M row reads and **100K row writes per day**.
  - Paid ($5/month): 50M writes/month and 25B reads/month included.

| | Low | Base | Breakout |
|---|---|---|---|
| Crawler (GH Actions) | $0 | $0 | $0 |
| R2 storage and egress | $0 | $0 | ~$0.50 |
| Worker + D1 | $0 (free tier) | $0 (peak ~15K writes/day) | **$5** (150K writes/day exceeds the free cap; Paid covers 4.5M/month) |
| **Total / month** | **$0** | **$0** | **≈ $5–10** |

Backups and history are free: every snapshot is a signed, immutable release.

### B. ICP (canisters)
Costs use the 13-node subnet table: 1T cycles ≈ $1.37, storage ≈ $0.45/GiB-month, queries free.
- **Ingest:** each batch ≈ 30M cycles (base 5M, ingress 1.2M, ~2 KB payload ≈ 4M, plus execution).
- **Crawl:** HTTPS outcalls, v2 pricing ≈ 49M cycles per ~15 KB skill file, replicated across 13 nodes.

| | Low | Base | Breakout |
|---|---|---|---|
| Storage (index + counters, ~5 GB; history means extra copies) | ~$3 | ~$4 | ~$6 |
| Signal ingest | ~$0.50 | ~$4 | ~$37 |
| Daily incremental crawl (~26K fetches/day) | ~$50 | ~$50 | ~$50 |
| One-off full crawl (1.3M fetches) | ~$90 | ~$90 | ~$90 |
| Development and ops cycles (reinstalls, upgrades, snapshot backups) | **$20–100** (the sibr.online experience) | same | same |
| **Total / month (excluding full crawl)** | **~$75–155** | **~$80–160** | **~$115–195** |

The cost isn't the only problem with the ICP crawler:
- **Rate limits:** a fully replicated outcall makes 13 requests, consuming GitHub's rate limits 13× faster.
- **Secrets:** an API token stored in a canister is visible to node operators.
- **Determinism:** responses need deterministic transforms before the subnet can agree on them.

Where ICP *is* strong: free queries, tamper-evidence, and (later) the option of on-chain incentives for feedback.

### C. Supabase or Neon instead of D1
- **Supabase free tier** pauses after 1 week of inactivity, so an always-on ingest endpoint needs Pro at **$25/month from day one**.
- **Neon free tier:** scales to zero (cold starts) with 0.5 GB per project; it would need Hyperdrive from Workers.
- Either is viable later if a sponsor appears. Neither beats D1 for the starting workload.

## 4. Recommendation
1. **Start with architecture A.**
   - It costs **$0 in the low and base scenarios** and **~$5–10/month at breakout scale**.
   - Backups are free and automatic (release history).
   - There is no idle-pause risk.
2. **Use ICP as a trust anchor only.**
   - Write the daily snapshot's root hash to a canister: ~1 update/day ≈ **< $0.01/month**.
   - This makes the public ledger tamper-evident without paying ICP prices for the crawl or ingest workload.
   - Revisit on-chain incentives for feedback once real signal volume exists.
3. **Design for the D1 write cap from day one:** client-side aggregation, per-(section, day) upsert counters, and snapshot-served reads.
4. **Apply to sponsorship programs** once traction is visible: Vercel OSS ($3,600 credits over 3 years), Cloudflare startup or OSS programs, and Neon.

**The adoption risk is product, not infrastructure.** Discovery-only tools stall (skillselion). Girth must lead with *measured savings and better results*, and the ledger must make results visibly better: recommendations that improve with signals, and taste and variation memory.

## Sources
- GitHub API (stars, created dates), queried 2026-10-14
- npm downloads API (`api.npmjs.org/downloads/point/last-month/...`), queried 2026-10-14
- skills.sh leaderboard: https://skills.sh/
- D1 pricing: https://developers.cloudflare.com/d1/platform/pricing/
- D1 limits: https://developers.cloudflare.com/d1/platform/limits/
- D1 free-tier enforcement: https://developers.cloudflare.com/changelog/post/2026-09-01-d1-free-tier-limit-enforcement/
- Workers pricing: https://developers.cloudflare.com/workers/platform/pricing/
- Workers Rust: https://developers.cloudflare.com/workers/languages/rust/
- ICP cycle costs: https://docs.internetcomputer.org/references/cycle-costs/
- Supabase / Neon / Turso free tiers: https://neon.com/guides/neon-vs-supabase-free-plan, https://agentdeals.dev/database-free-tier-comparison-2026
- Vercel Open Source Program: https://vercel.com/open-source-program
- Skill ecosystem state: https://www.openagentskill.com/reports/state-of-agent-skills-2026, https://www.skillsbench.ai/leaderboard
