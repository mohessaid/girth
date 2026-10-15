# Landscape scan and directions (2026-10-15)

A research spike. The output is findings and ranked directions, not code. Sources are listed at the end. Secondary sources are marked; numbers from them are indicative only.

## 1. What the research says about skills

| Finding | Source | What it means for Girth |
|---|---|---|
| Compact, focused skills gain **+18.9 pp**; comprehensive documentation gains **+5.7 pp**. **2–3 skills per task** give +20.0 pp; 4 or more give +5.2 pp. | SkillsBench (arXiv 2602.12670) | Independent evidence for Girth's thesis: a small slice beats the whole library. Cite it in the README. |
| Self-generated skills (written without a source) give little or no benefit; curated ones do. | SkillsBench; SkillLearnBench (arXiv 2604.20087): the best method closes only about 45% of the gap to human-written skills, and self-feedback alone causes drift. | Girth distils *curated* sources verbatim and refuses to write from nothing (the `no-source` eval). That's the right side of this result, but it should be shown on a public benchmark rather than asserted. |
| Hiding the skill *body* and routing on metadata only drops routing accuracy by **31–44 pp** across about 80K skills. | SkillRouter (arXiv 2603.22455) | `girth search` ranks on full text and headings, not descriptions. That is a real advantage over description-only listings. |
| Agentic hybrid search (iterative queries plus checking candidates) works best for retrieval in the wild. Claude's SkillsBench score went from 40.1% to 48.2% with refinement. | "How Well Do Agentic Skills Work in the Wild" (arXiv 2604.04323) | Girth's search → show → distil loop is this pattern. |
| Paired with/without trials ("Skill Lift"): mean lift 0.21, positive in 72.8% of cases. Static scans and LLM-judge scores barely correlate (ρ = 0.14). | ACES (arXiv 2608.20614), NVIDIA SkillEvaluator | Usage isn't utility. Stars and scans don't measure value; paired outcomes do. This is the metric the hosted index should rank on. |
| Skill libraries need a lifecycle: evidence, proposal, verification and admission, storage, retrieval, maintenance, distillation, governance. Flat retrieval degrades as libraries grow. | Dynamic Agent Skills survey (arXiv 2607.10113); SkillsVote (arXiv 2605.18401): a million-scale corpus with evidence-gated updates | Girth already covers proposal (craft), admission (`validate`), evidence (the ledger) and distillation. Maintenance (source drift) and governance (security) are the gaps. |

## 2. Security: skills are the new supply chain
- **Malicious skills at scale:** on ClawHub, 341 malicious skills were found early in 2026, and later counts exceed 1,184 (Koi Security, Antiy CERT). Scanners missed some after screening was added (Unit 42).
- **Payload-less attacks:** natural-language "compliance rules" in skill files had a 0% scanner detection rate (secondary source).
- **Standards:** OWASP published an Agentic Skills Top 10 (2026-04-27).
- **What it means for Girth:** Girth already reads third-party skills and never executes them. Distilling into a small, reviewed, source-marked skill is a sanitising step that a raw `npx skills add` lacks.

## 3. Claude Code platform changes (primary: the changelog and docs, v2.1.287–2.1.300)
- **Mods (v2.1.287+)** are JS/TS hook modules in a plugin. Three capabilities matter for Girth:
  - `prompt.attachment` with `type: "skill_listing"` can **rewrite the skill listing the model reads every turn**.
  - `skill.prompt` can rewrite a skill's text when it expands. It receives the source, plugin, path and frontmatter (v2.1.299+).
  - `turn.step` and `turn.complete` expose **exact token usage per request and per turn**, and `turn.step` can **change the model per request**. `agent.spawn` can choose a subagent's model.
  - A mod can also show its own panes and a band above the prompt, and run local binaries with `$.process.run`.
- **Built-in `/skill-doctor` (around v2.1.261)** lists never-invoked skills. `skillOverrides` (`name-only`, `user-invocable-only`) prunes skills reversibly. Its known blind spots (a secondary write-up):
  - it doesn't know a skill's age;
  - it doesn't flag plugin skills;
  - it treats skills that are dormant on purpose as dead;
  - it works per skill, not per section.
- **Haiku 5.5:** $0.10 / $0.50 per Mtok, about 40× cheaper than Opus 5.5 at $4 / $20.
- **Practitioner reports:** skill listings cost about 3K tokens per turn at 40 skills, and unused skills can account for about 18% of a day's tokens (secondary sources).

## 4. Wider AI
- **Prices:** per-token prices keep falling (BenchLM index at 16 vs a 2023 base of 100). The conversation is moving from cost per token to **cost per completed task**, which is the metric Girth reports.
- **Model routing:** routing and cascades (RouteLLM, FrugalGPT) report 40–85% savings on benchmarks and 20–60% in production at the same quality (mostly secondary). This is the layer above the harness.
- **Agent memory:** an active research area (Memora, O-Mem, MemMachine), with little that targets coding agents or *preferences per deliverable type*. Taste memory sits in that gap.
- **Agent protocols:** AGENTS.md is read by Claude Code; Meta and Sierra announced a Personal Agent Protocol; enterprise agent platforms add spend caps. Cost governance is becoming a product category.

## 5. Directions, ranked
Ranked by how much each lowers cost at equal quality, how defensible it is, and how much it costs to build.

1. **Lens: a per-project skill listing (mod).**
   - Rewrite `skill_listing` so the model sees only the skills this project uses, the crafted `girth-*` skills and Girth itself. The rest collapse into one line ("N more installed; `girth search` finds them").
   - Uses ledger evidence: first-seen date (so a new skill isn't treated as dead), uses per project, and plugin skills included.
   - Reversible, with no settings edits.
   - Beats `/skill-doctor` on each of its blind spots. Expected effect: always-on cost down about 90% with 40 skills installed.
   - Risk: a hidden skill is needed and missed. Mitigate with the pointer line plus a trigger eval.
2. **Meter: exact cost, measured continuously (mod).**
   - `turn.complete` usage goes into the ledger, so every session records real tokens and dollars, tagged with the listing and crafted skills in effect.
   - A small band shows the session cost and what Lens saved.
   - This turns Girth's claims into live, local measurement (ACES-style lift without a lab), and it is the data the hosted index needs.
3. **Cheap distillation.** Run the craft step on Haiku 5.5 through a forked subagent or a skill `model` override. Session 1 currently pays $0.22 to craft. Gate it on the existing evals, which must stay at 1.00.
4. **Shield: distil, don't install.** `validate` flags injection-shaped lines in crafted skills: instructions to fetch, execute or exfiltrate, or to hide actions from the user. This is cheap to build and fits the launch story.
5. **Proof on a public benchmark.** Run a SkillsBench subset with three arms: no skills, full curated skills, and Girth-distilled. This answers the "self-generated skills don't help" objection with numbers.
6. **Later, the next layer up:** routing models per step (`turn.step`), and Radar.

**Recommendation:** build 1 and 2 together as v0.5, as a mod inside the existing plugin. The mod stays thin JS; the decisions come from the Rust binary (`girth lens --json`), computed once per session.

## 6. Probe: what the skill listing actually costs (measured 2026-10-15)
An observe-only mod hooked `prompt.attachment`, `prompt.section` and `turn.step` in a real headless session (Claude Code 2.1.300, the author's real configuration, prompt "say hi in two words").

- **Size:** the `skill_listing` attachment held **74 skills in 29.3 KB, about 7.3K tokens**. It was the largest single attachment; the next were the MCP instructions (13.9 KB) and the deferred-tools delta (7.1 KB).
- **Truncation:** the last entries are names only, so Claude Code already drops descriptions once the listing runs past its budget. Those skills are listed but undescribed, which makes them unlikely to trigger.
- **Share of the prompt:** the request's input was about 35.2K tokens (8,605 read from cache plus 26,631 written to it), so the listing is about 21% of a minimal prompt.
- **Money at Opus 5.5 prices** (cache write $5/Mtok, cache read $0.40/Mtok):
  - about $0.037 per session to write the listing to the cache;
  - about $0.003 per turn to read it back;
  - each subagent and each cache expiry pays the write again.

  The direct dollar saving is modest per turn. The larger effects are focus (SkillsBench: 4+ skills degrade results) and triggering: undescribed or crowded skills misfire or never fire.
- **Constraint found:** `claude plugin validate` reports that `prompt.attachment` and `prompt.section` hooks do **not run for plugins a person installs** on machines where an organization manages Claude Code. Lens works for individuals, not managed enterprise seats, unless the org lists it.
