# Would trimming tool results pay? (2026-10-26)

**Question.** A mod's `session.append` hook can rewrite a `tool_result` before it is stored, so a trim never breaks the cache. Is there enough in large tool results to make that a Girth lever?

**Data.** The author's main-thread transcripts, 2026-10-10 to 2026-10-26: 277 sessions, 44,899 requests. Each block is weighted by the number of later requests that carry it. This is the method behind the 13.8B figure in `docs/research/2026-10-15-where-the-money-goes.md`, which covered a longer window that transcript retention has since dropped. The [RTK](https://github.com/rtk-ai/rtk) hook was on throughout and rewrites shell commands to filter their output. `rtk gain` reports 860M raw tokens cut to 171M (−80%) over 71,350 commands.

```
$ python3 examples/context/analyze-context.py  # conversation re-reads by kind
main requests 44,899 · conversation tokens re-read 3.40B (≈ $680 at the Opus read rate)
$ python3 examples/context/analyze-tails.py
tool tokens re-read 3.08B · results over 2,000 tokens: 8.7%
kind                                blocks  share    p50    p95     max  over 2K  carried
result: Bash                        37,139  39.0%     90   1030    7383    13.1%      140
input: Bash heredoc                 11,544  32.1%    404   1659   10432    14.5%      143
input: Bash                         25,597  10.4%     73    212    1383     0.0%      139
input: Write                         1,378   7.6%   1088   4759   51017    57.1%       99
result: Read                         1,541   1.8%      0   2395   14315    78.2%      126
result: WebFetch                       249   0.5%    325   3060   11221    48.5%       97
```

**Findings.**
- On this machine, tool results are already small: a Bash result's median is 90 tokens and its p95 about 1K. Results over 2K tokens are 8.7% of re-read tool tokens.
- Halving every one of them would save about 4% of tool re-reads, roughly $27 over these 16 days against $680 of conversation re-reads.
- **That is RTK's doing.** Without a filter, shell output would be about 5× larger, and Bash results would be most of the tool traffic. For a user without one, large tool output is a real contributor.
- `result: Read` has p50 = 0 because half of these Read calls returned images (screenshots, video frames), and the scripts count only text. Read is undercounted; images cost about 1.5K tokens each.
- Half of the tool traffic is **what Claude wrote**, not what tools returned. Bash commands carrying a heredoc (file writes) take 32%, and `Write` inputs 8%. `session.append` can rewrite only text blocks and `tool_result` content, so a mod cannot trim them.
- 87% of all re-read content was more than 20 requests old when it was read. Age is what costs, not the size of any one result.

**Decision.**
- **No trimming mod in Girth.** A shell-output filter already exists and works (RTK, −80% here). A second one in Girth would change what Claude sees and duplicate it.
- **Detect it instead.** `girth advise` should gain a contributor, *large tool output*: when tool results are a large share of the window's input, the fix is a shell-output filter such as RTK, or `| tail` / `| head` in commands. The guard is that the filter keeps errors and summaries. This needs per-turn tool-result sizes in the ledger.
- The lever for old context is already shipped and recommended by `girth advise`: `/clear` on a task switch, `/compact` at a break, and opt-in idle compaction. Its outcome A/B is the open item.
