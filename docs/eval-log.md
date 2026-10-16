# Eval log

Behavioural suite run with `make evals` (`claude plugin eval` on the staged dist plugin, with the release binary bundled at `skills/girth/bin/girth`).

| Date | Model | Case | Runs | Score | Cost |
|---|---|---|---|---|---|
| 2026-10-14 | claude-sonnet-5-5 | craft-captions | 3 | 1.00 | $0.20 |
| 2026-10-14 | claude-sonnet-5-5 | tone-copy | 3 | 1.00 | $0.20 |
| 2026-10-14 | claude-sonnet-5-5 | no-source | 3 | 1.00 | $0.15 |

What the graders check:
- **craft-captions:** the skill fired; `girth search` ran; `crafted-skills/girth-captions/SKILL.md` exists; `VALID girth-captions` appears; the "1.2 seconds" rule is kept; colour grading, LUT, export and LUFS content is dropped.
- **tone-copy:** tone guidance is kept verbatim; email cadence and ad bidding are dropped; the skill validates.
- **no-source:** nothing is written under `crafted-skills/`, and the reply says no source was found (LLM judge, 3/3 PASS each run).

Sample crafted output (craft-captions, run 1):

```markdown
---
name: girth-captions
description: Use when timing, splitting or line-breaking captions for promo videos in this project.
metadata:
  girth:
    sources:
      - path-or-repo: <workspace>/vendor-skills/video-mega
        sections: [Caption timing]
    crafted_for: caption timing rules in our promo videos
    created: 2026-10-14
    last_used: 2026-10-14
---
# Caption timing

- Never show more than 2 lines of captions at once.
- Each caption stays on screen for at least 1.2 seconds.
- Always break lines at natural phrase boundaries, never mid-phrase.
```

## 2026-10-14: v0.3 taste memory

`make evals` (claude-sonnet-5-5, 3 runs per case) on the release tree: **4 of 4 cases at 1.00, $0.93.**

| Case | Score | Cost |
|---|---|---|
| craft-captions | 1.00 | $0.23 |
| no-source | 1.00 | $0.16 |
| taste-launch (new) | 1.00 | $0.29 |
| tone-copy | 1.00 | $0.25 |

What `taste-launch` checks:
- The scaffold seeds a project-tier house rule (an end card with `girth.dev`), plus three earlier launch videos that all used `neon-on-black` / `synthwave`.
- In run 1, the model picked paper-ink and minimal-techno, said why ("your last video used neon-on-black and synthwave"), put girth.dev on the end card, and recorded its choices with `girth made`.

**Manual end-to-end check** (claude-haiku-5-5, a plugin-style hook calling the real launcher, an isolated `GIRTH_HOME`):
- The hook injected the taste, and the model applied the house rule.
- The first run refused to execute the `made` line, because it appeared only in hook output. The crafted-skill template gained a `## Record` section for this reason.
- With that section, the model recorded `mood=hopeful` and explicitly varied away from the last recorded `melancholy`.

## 2026-10-15: M2 (self-trigger, fetch, show)

`make evals` on the release tree: **7 of 7 cases at 1.00 (21 runs), $1.66.**
- New: `auto-captions`, `no-fire-generic`, `extend-captions`.
  - The with-only graders show Girth firing unprompted on the specialised task in **3 of 3** runs, and **0 of 3** on the generic one.
  - Ablation (`--ablation with-without`) gives Δ = 0 on output quality for these fixtures. The source skills sit inside the fixture project, so a capable model without Girth reads them directly. Girth's measured value on capable models is cost at equal quality (`examples/compounding/RESULTS.md`), not a quality lift. Hiding sources from the baseline only would manufacture a Δ, so it isn't done.
- Regression caught and fixed: after `girth show` landed, `tone-copy` fell to 1 of 3. The model read the unlabelled heading list as the whole skill. The overview is now labelled, and `tone-copy` is back to 3 of 3.

## 2026-10-15: v0.5 (Lens, Meter)

`make evals` after the SKILL.md change: **7 of 7 cases at 1.00 (21 runs), $1.45.**
- The one-line SKILL.md addition about "Also installed" skills changed nothing measurable.
- Lens's behavioural proof is a headless A/B, not a plugin-eval case, because mod loading under `claude plugin eval` is unconfirmed.
  - The A/B ran with the author's real 74-skill setup; a plugin-eval sandbox has neither a real listing nor a ledger.
  - **Results:** −19% cost, with the collapsed matching skill still invoked in 3 of 3 runs (`examples/lens/RESULTS.md`).
- **Tooling note:** run `make dist` with the normal toolchain first, then `PATH=/usr/bin:/bin:$PATH make -o dist -o build evals`. A system Cargo earlier on the PATH can't build edition 2024.

## 2026-10-15: v0.6 (MCP Lens, X-ray)

`make evals` on the release tree: **7 of 7 cases at 1.00 (21 runs), $1.51.**
- No SKILL.md change in this release; the suite confirms the skill behaves as before with the new mod loaded.
- MCP Lens and X-ray are proved by headless runs with the author's real setup, not by plugin-eval cases:
  - **Cost A/B:** −29% against v0.5 and −42% against Lens off, at equal quality (`examples/lens/RESULTS.md`).
  - **Hidden servers:** 6 of 6 (Atlassian with held-back instructions, Excalidraw with collapsed tools).
  - **`/girth-xray`:** a two-turn `claude -p` session on the dist plugin (isolated `GIRTH_HOME`) printed the request composition, with the skill listing at 7,918 B (Lens: was 30,054 B) and 26,222 B removed per request in total.

## 2026-10-15: v0.7 (cache keeper and doctor)

`make evals` on the release tree: **7 of 7 cases at 1.00 (21 runs), $1.44.**
- No SKILL.md change. The keeper's 55-minute timers never fire inside an eval run.
- The keeper and doctor are proved live (`examples/cache/RESULTS.md`):
  - the 66-minute probe: the return cost 81% less;
  - an isolated end-to-end run: pings, cancellation, a real break recorded.

## 2026-10-16: v0.8 (idle compaction, subagent model)
`make evals` on the release tree: **7 of 7 cases at 1.00 (21 runs), $1.45.**
- No SKILL.md change. Both features are opt-in, and idle compaction can't run in `claude -p`.
- **Proved live on the dist build** (interactive tmux, Haiku 5.5, 20 s interval, isolated GIRTH_HOME):
  - a compaction from 38,668 to 9,096 tokens; the next answer still knew the codename, with the line under it;
  - a turn inside the interval cancels the pending compaction;
  - with `agents.model: haiku`, a general-purpose subagent of a Sonnet session ran on Haiku.
- **Post-release checks** (`claude -p`, Sonnet main, dist build):
  - With no `agents` config, an Explore subagent ran on the parent's model (Sonnet). It has no Haiku default of its own here, so `agents.model` doesn't upgrade it.
  - A project agent whose frontmatter says `model: opus` ran on Opus even with `agents.model: haiku`. The agent's own choice is kept.

## 0.9.0 (2026-10-16)
- **Live run A** (dist build, Haiku 5.5, isolated `GIRTH_HOME`, $0.004):
  - unpiped `cargo test` (failing), a fix, then `cargo test 2>&1 | tail -3`, then `git commit`;
  - the turn recorded `"outcomes":["test_fail","test_pass","commit"]`. The first verdict came through core's `isError` path, which the mod tests can't fake.
- **Live run B** ($0.005): a general-purpose subagent ran the piped tests.
  - Its turn recorded `["test_pass"]`; the main turn recorded no outcomes.
  - The output had been rewritten by the author's RTK hook (`cargo test: 1 passed (2 suites, 0.00s)`) and was still read correctly. RTK keeps cargo's own `test result: FAILED` line for failures.
- **The ledger held no command text** after both runs (`cargo`, `tail -3` and `lib.rs`: 0 matches). The real `~/.girth` was unchanged.
- **Evals (0.9.0 dist):** 7 of 7 cases at 1.00, 3 runs each, 381 s, $1.46.
- **Live run C, after review fixes** ($0.004):
  - `cargo test 2>&1 | grep -E 'test result'` (failing), a fix, then `cargo test 2>&1 | grep -E 'FAILED|panicked'` (passing; grep printed nothing and exited 1), then `git commit -am`;
  - recorded `["test_fail","test_untold","commit"]`. The grep-filtered passing run is no longer a false failure.
