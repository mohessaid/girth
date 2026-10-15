# Launch announcement: DRAFT (not posted)

Built from the author's own words in the project kickoff. Edit freely. The numbers are measured and linked; keep them as they are or re-run them.

---

## Short post (X / Bluesky / LinkedIn opener)

I kept hitting the same wall: I install dozens of skills, my agent uses a fraction of each, and I pay for all of them every turn.

So I built **Girth**. When your agent needs domain know-how, Girth distils only the slice your project needs from the skills you already have into one small project skill. Later sessions reuse it.

Measured over 5 sessions in one domain:
- every skill installed: $0.91
- Girth: $0.60 (**−34%**), with every output passing the same check.

Open source, Rust, one binary, no telemetry unless you opt in.
`/plugin marketplace add mohessaid/girth`
github.com/mohessaid/girth

---

## Longer post

### The skill-nomy is running in a void loop

Skills ship faster than anyone can read them. Some are great; others are noise, duplicates, rip-offs or customisations. GitHub stars stopped meaning much in the AI era, and nobody shares feedback about the skills they actually use. So there is no metric for which skill is the right one, and everyone installs everything just in case.

That costs twice:
1. **Money and context.** Every installed skill's description is loaded every turn, and the big ones get read whole when only one section matters.
2. **Results.** In all my projects I ask for a launch video in motion graphics, but I always want something different added. With the skills installed, the videos started to feel generic. And when I asked for something else, I didn't know what reached the model, but it lost everything I'd told it before.

### What Girth does

Girth is a skill plus a small Rust CLI for Claude Code, and for any agent the skills CLI supports.

- **Just-in-time skills.** When a task needs specialised know-how, Girth searches the skills already on disk, prints only the sections that matter, and the model distils them into `.claude/skills/girth-<domain>`: rules kept verbatim, tone never compressed, under 3 KB. Next time it's a cache hit.
- **Taste memory.** House rules you stated once ("always end on the logo with the URL") are applied every time, in every project. What your last deliverables chose is remembered too, so the next one varies unless you pin it.
- **A ledger, not stars.** Girth records locally which skills you actually use, and which sections your crafted skills keep or drop. Sharing is opt-in, and only content hashes plus counts would ever leave your machine.

### Numbers (one machine, single samples, all reproducible)

| | 5-session total |
|---|---|
| Every skill installed | $0.909 |
| Fresh distillation every session | $1.061 |
| **Girth** | **$0.599** |

Session 1 pays to craft. Sessions 2–5 cost about half of a bulk session. The tasks never mention Girth: it triggers itself (3 of 3 on specialised tasks, 0 of 3 on generic ones). Full method: `examples/compounding/RESULTS.md`.

**What it doesn't claim:** better output than a capable model reading the full skills. The claim is the same quality for less.

### What's next

Girth starts at the harness layer and moves up the stack. Next is a shared index where skills are ranked by what people keep, not by stars. That only works if you opt in.

Try it, break it, tell me what it got wrong: github.com/mohessaid/girth
