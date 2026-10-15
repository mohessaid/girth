# Lens A/B (2026-10-15)

**Setup** (`ab.sh`, `summarize.py`):
- The author's real Claude Code 2.1.300 setup: 74 skills listed, 30,027 bytes.
- The model is `claude-sonnet-5-5`. Each run happens in a fresh temp dir.
- The Girth dist plugin loads with `--plugin-dir`, and the installed copy is disabled for the run.
- Each run uses a copy of the author's ledger in its own `GIRTH_HOME`; the real `~/.girth` was diffed afterwards and was unchanged.
- **Task** (`task.md`): a one-page bakery site with complete SEO and social metadata. The author has a matching skill (`seo-aeo-best-practices`) that their ledger has never seen used, so **Lens collapses it**. That makes this a hidden-skill test.
- **Check:** 6 metadata markers in `index.html`.

## Result (3 runs per arm)
| run | cost $ | input+cache write | cache read | metadata checks | skills invoked | listing bytes |
|---|---|---|---|---|---|---|
| off-1 | 0.170 | 32,505 | 78,036 | 6/6 | seo-aeo-best-practices | 30,027 |
| off-2 | 0.164 | 31,544 | 77,041 | 6/6 | seo-aeo-best-practices | 30,027 |
| off-3 | 0.164 | 32,028 | 79,453 | 6/6 | seo-aeo-best-practices | 30,027 |
| on-1 | 0.133 | 24,569 | 65,939 | 6/6 | seo-aeo-best-practices | 7,869 |
| on-2 | 0.132 | 24,507 | 65,954 | 6/6 | seo-aeo-best-practices | 7,869 |
| on-3 | 0.141 | 25,103 | 65,963 | 6/6 | seo-aeo-best-practices | 7,869 |

- **Cost:** −19% (mean $0.135 vs $0.166).
- **Prompt tokens:** fresh tokens (input + cache write) −23%; cache reads −16%.
- **Quality:** equal, 6/6 in every run.
- **Hidden skill:** the collapsed skill was still invoked in 3 of 3 runs, the same as with Lens off.

## What the pilot found first
The first version of Lens collapsed skills to **bare names**. In a pilot (1 run per arm), the Lens-off run invoked `seo-aeo-best-practices` and the Lens-on run did not. The output still passed the 6 checks, because a capable model knows SEO, but the skill was lost. This matches SkillRouter (arXiv 2603.22455): routing on names alone loses 31–44 points of accuracy.

Lens now keeps a **short hint** for each collapsed skill: the first ≤ 60 characters of its description at a word boundary, with a leading "Use when" dropped. The listing cut went from 87% to 74%, and routing came back.

## Caveats
- One task and 3 runs per arm on one machine. The direction is clear; the size will vary with the setup.
- The 6-marker check is coarse: it shows the deliverable is complete, not that it's better.

# v0.6: Lens for MCP and tools (2026-10-15)

**Same setup and task** as above. The arms are `skills` (v0.5: the skill listing only) and `on` (v0.6: the skill listing, MCP server instructions and the deferred-tools list), 3 runs each.

| run | cost $ | input+cache write | cache read | metadata checks | skills invoked | Lens bytes before→after |
|---|---|---|---|---|---|---|
| skills-1 | 0.137 | 24,289 | 66,775 | 6/6 | seo-aeo-best-practices | 30,027→7,869 |
| skills-2 | 0.135 | 24,658 | 65,963 | 6/6 | seo-aeo-best-practices | 30,027→7,869 |
| skills-3 | 0.136 | 25,332 | 64,208 | 6/6 | seo-aeo-best-practices | 30,027→7,869 |
| on-1 | 0.095 | 15,750 | 50,717 | 6/6 | seo-aeo-best-practices | 38,260→9,815 |
| on-2 | 0.095 | 15,324 | 50,634 | 6/6 | seo-aeo-best-practices | 34,218→9,852 |
| on-3 | 0.099 | 16,604 | 51,411 | 6/6 | seo-aeo-best-practices | 34,087→9,799 |

- **v0.6 vs v0.5:** −29% cost (mean $0.096 vs $0.136) and −36% fresh prompt tokens, at equal quality.
- **v0.6 vs Lens off** (mean $0.166 in the runs above): **−42% cost.**
- The "before" bytes vary between runs because the MCP and tools deltas depend on which servers had connected when the request was sent.

## Hidden servers (3 runs each, Lens on)
- **Instructions held back, Atlassian (`task-jit.md`, read-only):** 3 of 3 passed.
  - In every run the first Atlassian call received the held-back instructions, Claude then followed them (`getAccessibleAtlassianResources` before anything else, as the server asks), and the read succeeded.
  - This is the case where silently dropping the instructions would have broken the flow.
  - The account details in the answers stay in local logs and are not reproduced here.
- **Tools collapsed, Excalidraw (`task-tools.md`, read-only):** 3 of 3 passed. Claude found the collapsed tools through ToolSearch and `read_me` succeeded.
