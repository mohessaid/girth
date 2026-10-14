# Sharing consent

Ask once, with AskUserQuestion:

> **Help Girth recommend better skills?** Girth can share anonymous counts: which public skill sections were used, kept or removed, plus 👍/👎, as content hashes. **Never** names, paths, text, notes or anything from your project. Nothing is uploaded until the Girth index launches.

The options are **Allow**, **Don't allow** and **Show me exactly what would be shared**.

- Allow → `scripts/girth share allow`
- Don't allow → `scripts/girth share deny`
- Show me → run `scripts/girth share preview` and show the JSON. Explain that each `h` is a hash of a public skill section, then ask again with Allow or Don't allow.

Afterwards, only change it when the user asks ("girth sharing off", and so on), using the same commands.
