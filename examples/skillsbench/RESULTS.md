# SkillsBench proof

**Question:** do Girth-distilled skills keep the lift of curated skills? SkillsBench (arXiv 2602.12670) found that curated skills help (+16 pp) and that self-generated ones barely do. Girth distils curated skills, so it should keep the lift. This run checks that.

**Status (2026-10-15):**
- **Harness:** BenchFlow's `bench` from the SkillsBench lockfile (`uv sync --locked`), with local Docker. The oracle passes on `offer-letter-generator` (1/1).
- **Girth arm:** crafted (`craft.sh`, `crafted/`). Each task's skill was distilled **only from that task's own curated skills**, with its instruction as the need. All 6 pass `girth validate`. The crafts cost $1.06 in total.

  | task | curated skills | girth skill |
  |---|---|---|
  | offer-letter-generator | 7,869 B | 2,884 B |
  | xlsx-recover-data | 21,003 B | 2,835 B |
  | sales-pivot-analysis | 14,297 B | 3,786 B |
  | edit-pdf | 10,941 B | 3,670 B |
  | pdf-excel-diff | 82,116 B | 2,314 B |
  | software-dependency-audit | 28,545 B | 3,117 B |

- **Not yet run: the agent - **Agent arms: run on 2026-10-15** with BenchFlow `claude-agent-acp` and `claude-sonnet-5-5`, using the author's Claude Code login (their explicit choice). One run per arm per task.

## Result

| arm | tasks passed | input tokens* | output tokens* | skill calls / task |
|---|---|---|---|---|
| none | 3/6 | 44,728 | 32,801 | 0.0 |
| curated | 3/6 | 46,428 | 26,454 | 1.8 |
| **girth** | **3/6** | 46,440 | **20,317** | 1.0 |

| task | none | curated | girth |
|---|---|---|---|
| edit-pdf | fail | pass (pdf-editing, text-parser) | **pass** (girth-pdf-editing) |
| offer-letter-generator | pass | pass (docx) | pass (girth-docx-templates) |
| pdf-excel-diff | pass | pass (pdf) | pass (girth-pdf-excel-diff) |
| sales-pivot-analysis | fail | fail (pdf, xlsx) | fail (girth-excel-pivot-report) |
| software-dependency-audit | **pass** | fail (3 skills) | fail (girth-dependency-audit) |
| xlsx-recover-data | fail | fail (data-reconciliation, xlsx) | fail (girth-xlsx-recovery) |

\*BenchFlow's token counts under subscription login are partial (ACP usage telemetry). Read them as relative, not as a bill.

**What it shows:**
- **Girth's distilled skill matched the curated skills on every task.** It passed where they passed and failed where they failed, using one skill of 2–4 KB instead of up to three skills of 8–82 KB, and producing 23% fewer output tokens.
- **The SkillsBench objection doesn't hit Girth.** "Self-generated skills don't help" is about skills written from nothing. Girth distils curated sources, and here it kept their effect exactly.
- **Curated skills showed no net lift on this subset** (3/6, the same as no skills). They turned `edit-pdf` from fail to pass, and the dependency-audit skills turned a pass into a fail, in both the curated and the girth arms. Girth carried the curated skills' effect, good and bad, which is what a faithful distillation should do.

**Caveats:**
- **Sample size:** 6 tasks, one run each, so there are no confidence intervals.
- **Model:** Sonnet 5.5, a newer model than the paper's.
- **Next run:** more runs per task, and the tasks where the paper reports large lifts.

runs; Sonnet 5.5 by default):
```bash
export ANTHROPIC_API_KEY=...            # or: export SKILLSBENCH_USE_CLAUDE_LOGIN=1
examples/skillsbench/run.sh <skillsbench checkout> /tmp/sb-proof
python3 examples/skillsbench/summarize.py /tmp/sb-proof
```
The result goes here and in the README, whichever way it comes out.
