# Taste memory

Girth keeps house rules and recent choices per domain, so recurring deliverables (launch videos, decks, landing pages) follow the user's lasting preferences while varying everything else. It is stored locally and never shared.

## Remember
When the user states a lasting preference ("always", "never", "every time", "from now on"), run `scripts/girth taste add <domain> "<rule>"` and confirm in one line ("Remembered for launch-video: …"). Keep the rule's scope in its text ("when there is narration, …"). Add `--project` when it only applies to this project. Only record what the user said, never what you inferred.

## Forget
Run `scripts/girth taste show <domain>`, find the rule's number, then `scripts/girth taste drop <domain> <n>` (add `--project` for a project rule).

## Show
`scripts/girth taste` lists domains; `scripts/girth taste show <domain>` shows rules and the last values per key.

## Record a deliverable
After delivering, run `scripts/girth made <domain> key=value…` with the 3–6 choices that define it (e.g. `palette=neon-on-black music=synthwave opening=logo-glitch`). Reuse the keys already listed by `taste show`, so variation can be tracked.
