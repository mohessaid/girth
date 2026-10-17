# Girth intro: storyboard

Silent, 75 s, 16:9, 24 fps. Chalk on black, drawn by Blender's Line Art with subtle wobble. Every number is in `FACTS.md`.

**Promise:** Girth reads your big skills once, keeps only the slice your project needs, and reuses it.

**Arc:** hook → the weight → turn (girth) → mechanism (search, distil, keep, reuse) → proof → the rest of the bill → close → call to action → the wink.

**Transitions:**
- **Primary:** cut on camera motion. The camera is moving when the cut lands, and the next shot keeps moving the same way and settles.
- **Accent 1:** zoom-through into an object (into the manual, into the project folder).
- **Accent 2:** dip to black, used only for the turn and the outro.

Captions are short: at most 6 words per line, checked against ~2.6 words a second. The verdict lands 1–2 s after the title can be read.

**The director's pass** (`.claude/skills/girth-video/references/pacing.md`):
- Camera moves stay below the strobe line, and no hold freezes.
- The second half carries as much motion as the first.
- The zoom-through into the project box (about 32 s) is the mid-film rhythm break.
- Lens, meter and keeper speed up (3.4 / 3.0 / 2.8 s) instead of ticking at 3-3-3.

`film.py` is authored on the first cut's clock; the `retime([...])` edit list maps it onto the times below.

| # | Time | Shot A (states the idea) | Shot B (consequence and verdict) | Into the next |
|---|---|---|---|---|
| 1 Hook | 0.0–7.5 | The camera glides right along a shelf of skill files, labelled with real installed names. **"you installed dozens of skills."** | Still gliding: a prompt types `make a title card`, and every file on the shelf lifts at once. **"your AI carries all of them."** → verdict **"every turn."** | cut on motion, pushing forward |
| 2 The weight | 7.5–14.5 | The camera pushes into one file, `hyperframes`, and it unfolds into a long accordion of pages. **"the video skills alone:"** → **"1.7 MB of manuals."** | Cut: a `next session` tag, and the accordion unfolds again. **"next session, it reads them again."** → **"and again."** | dip to black |
| 3 Turn | 14.5–19.5 | "girth" is written by hand in chalk. **"read once. keep only the slice you need."** | | cut on motion, left |
| 4 Search | 19.5–25.5 | A scan line sweeps the shelf, and four real source skills lift out. **"it searches the skills you have."** | Cut: single pages rise out of them, each with its real heading. → **"only the slice."** | continuous move |
| 5 Distil | 25.5–33.0 | The pages fold into one small card, `girth-video / SKILL.md`. **"distilled into one small skill."** → **"3 KB."** (footnote, shown while the pages are on screen: four skills, 1.1 MB of docs) | The card slides into a `your-project/` folder and the lid closes. **"it lives in your project."** | zoom-through into the folder |
| 6 Reuse | 33.0–38.5 | The folder, tagged `next session`, opens; the card is already there. **"next session, it's already there."** | The card lifts and the shelf stays put. → **"nothing to reread."** | cut on motion, right |
| 7 Proof | 38.5–49.0 | Two rows of five chalk bars draw in, one per session: every skill installed, and girth. **"same tasks, five sessions."** The first girth bar is taller (it crafts). | The rest come in at about half; the totals land at **$0.91** and **$0.60** → **"−34%."** (footnote: measured on one machine. yours will differ.) | push left ×3 |
| 8 The rest of the bill | 49.0–58.2 | lens: a row of cards collapses to thin lines. **"lens: unused skills become one line."** · meter: a receipt prints. **"meter: what each commit cost."** → **"$3.28"** · keeper: a clock hand sweeps. **"keeper: your cache stays warm."** | | cut on motion |
| 9 Close | 58.2–63.5 | **"girth"** · **"local. measured. open source."** · `/plugin marketplace add mohessaid/girth` | | push up |
| 10 Call to action | 63.5–69.5 | A chalk sketch of the repo's GitHub header: `github.com/mohessaid/girth`, **mohessaid / girth**, its real description, and a **☆ Star** button. A cursor glides in and clicks it; the star fills and the button reads **Starred**. **"star it on GitHub,"** → **"so it's there later."** | The camera eases back. **"know someone paying for this? share it."** | match cut to the card |
| 11 The wink | 69.5–75.0 | The `girth-video` card from shot 5 returns. **"p.s. this video was made with girth."** | **"it remembered: chalk, calm lines, room to breathe."** A chalk face winks. | dip to black |
