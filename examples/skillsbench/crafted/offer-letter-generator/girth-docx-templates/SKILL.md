---
name: girth-docx-templates
description: Use when filling a Word .docx template's {{PLACEHOLDER}} fields from JSON data, including IF/END_IF conditional sections, headers/footers and tables (e.g. offer letters).
metadata:
  girth:
    sources:
      - path-or-repo: /tmp/claude-1001/-home-mohessaid-workspaces-ai-girth/953581f1-bdc3-41dc-8803-c46cf94e3dce/scratchpad/sb/skillsbench/tasks/offer-letter-generator/environment/skills/docx
        sections: [Critical: Split Placeholder Problem, Best Practice: Regex-Based Full Replacement, Headers and Footers, Nested Tables, Conditional Sections, Common Pitfalls]
    crafted_for: Fill an offer letter Word template with employee_data.json, handle IF_RELOCATION section, save to /root/offer_letter_filled.docx
    created: 2026-10-15
    last_used: 2026-10-15
---
## Split placeholders <!-- src: docx#critical-split-placeholder-problem -->
Word often splits `{{CANDIDATE_NAME}}` across several runs. NEVER match on `run.text`. Always work at paragraph level: read `para.text`, replace, write the result into `runs[0].text` (keeps first run's formatting) and set the other runs to `''`.

## Where to look <!-- src: docx#headers-and-footers, docx#nested-tables, docx#common-pitfalls -->
Process ALL of: `doc.paragraphs`; every table cell's paragraphs, recursing into `cell.tables`; and for each `doc.sections` entry `section.header.paragraphs` and `section.footer.paragraphs` (not in `doc.paragraphs`).

## Placeholder replacement <!-- src: docx#best-practice-regex-based-full-replacement -->
Pattern `\{\{([A-Z_]+)\}\}`; for each match whose key is in the data dict, replace with `str(data[key])`. Only rebuild the paragraph if the text changed. Keys in JSON may differ in case/naming from placeholders: check, and verify none remain.

## Conditional sections <!-- src: docx#conditional-sections, docx#common-pitfalls -->
`{{IF_X}}...{{END_IF_X}}`. Conditional markers must never be left behind. If included (e.g. RELOCATION_PACKAGE == "Yes"): strip only the markers, keep content, and still fill placeholders inside. If excluded: delete the content between markers.
Gotcha (own note): the source example only handles markers in the same paragraph. Markers are often in separate paragraphs (or on a paragraph of their own): then walk body paragraphs in order, and when including delete a marker-only paragraph (`p._element.getparent().remove(p._element)`) or strip the marker text; when excluding remove every paragraph from start marker to end marker inclusive. Also check inside table cells.

## Verify
Reopen the saved file; assert no `{{` or `}}` remains in body, tables, headers or footers, and that excluded content is absent / included content present. Save to the exact output path requested (create the directory if needed).

## Record
After each deliverable, run the taste context's `made` command with its defining choices.
