# SkillsBench proof: ready to run, not yet run

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

- **Not yet run: the agent arms.** BenchFlow's `claude-agent-acp` needs either an `ANTHROPIC_API_KEY` or permission to copy the author's Claude Code login (`~/.claude/.credentials.json`) into each task's container. Those containers are built from third-party Dockerfiles, so that is the author's decision. `run.sh` refuses to start until one is chosen explicitly.

**To run** (about 18 agent runs; Sonnet 5.5 by default):
```bash
export ANTHROPIC_API_KEY=...            # or: export SKILLSBENCH_USE_CLAUDE_LOGIN=1
examples/skillsbench/run.sh <skillsbench checkout> /tmp/sb-proof
python3 examples/skillsbench/summarize.py /tmp/sb-proof
```
The result goes here and in the README, whichever way it comes out.
