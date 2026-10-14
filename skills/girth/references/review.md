# Review a crafted skill

1. Read `<output_dir>/girth-<domain>/SKILL.md` and list its `##` sections.
2. Ask with AskUserQuestion (multiSelect): "Which sections should go?" Use one option per section: the label is the heading, the description its first rule. With more than 4 sections, ask in batches of 4.
3. Delete the chosen sections, from the heading to just before the next heading. Change nothing else.
4. Run `scripts/girth validate <dir>`. It records exactly what was removed.
5. Offer a rating (`references/feedback.md`).

If the user wants something *added* rather than removed, extend the skill as in step 4 of the main flow.
