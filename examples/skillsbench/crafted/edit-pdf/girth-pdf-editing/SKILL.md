---
name: girth-pdf-editing
description: Use when updating, filling or redacting an existing PDF from a text file of instructions or new values, with PyMuPDF, keeping the text layer and not covering original text.
metadata:
  girth:
    sources:
      - path-or-repo: /tmp/claude-1001/-home-mohessaid-workspaces-ai-girth/953581f1-bdc3-41dc-8803-c46cf94e3dce/scratchpad/sb/skillsbench/tasks/edit-pdf/environment/skills/pdf-editing
        sections: [CRITICAL RULES - READ FIRST, Finding Text Positions, Common Pattern: Fill Empty Form Fields, Replacing Existing Text (CRITICAL - MUST FOLLOW), TRUE REDACTION for Sensitive Data (IMPORTANT!), Workflow: Compare and Update]
      - path-or-repo: /tmp/claude-1001/-home-mohessaid-workspaces-ai-girth/953581f1-bdc3-41dc-8803-c46cf94e3dce/scratchpad/sb/skillsbench/tasks/edit-pdf/environment/skills/text-parser
        sections: [Key-Value Parsing, Tips]
    crafted_for: edit a PDF using an input.txt of instructions and updated info, write /root/output/output.pdf
    created: 2026-10-15
    last_used: 2026-10-15
---
## Rules <!-- src: pdf-editing#critical-rules---read-first -->
- Use Python `import fitz` (PyMuPDF). Never rasterize/flatten (`get_pixmap`, PDF to image to PDF).
- Never strikethrough. Never add new text NEXT TO an old value: replace at the SAME position (`rect.x0`, baseline `rect.y1`).
- Never black-fill redaction: `add_redact_annot(rect, fill=(1,1,1))` white only.
- Keep all form labels visible and text extractable. Font size ~10-12pt, match the neighbours.

## Read input first <!-- src: text-parser#key-value-parsing, text-parser#tips -->
Read all of input.txt and `page.get_text()` for every page. Lines may be `- Key: value` (strip leading `- `, `split(':',1)`). Match keys to PDF labels. Handle special instructions (redact, mask, date, signature). input.txt is the source of truth.

## Compare and update <!-- src: pdf-editing#workflow-compare-and-update -->
Build `{old_in_pdf: new}`; skip where equal (do not re-add correct info). Locate with `page.search_for(old)` (list of Rects: x0,y0,x1,y1); check each page and all hits.

## Replace text <!-- src: pdf-editing#replacing-existing-text-critical---must-follow, pdf-editing#true-redaction-for-sensitive-data-important -->
The source skill covers old text with a white `draw_rect`. This task forbids covering original text, so instead remove it: use a TIGHT rect (e.g. `Rect(r.x0, r.y0+2, r.x1, r.y1-2)` so nearby labels are untouched), then
```python
page.add_redact_annot(tight, fill=(1,1,1)); page.apply_redactions()
page.insert_text((r.x0, r.y1-2), new, fontsize=11, color=(0,0,0))
```
Call `apply_redactions()` once per page after all annots (it can shift/remove overlapping text). `draw_rect` only hides text visually; extractors still see it.
Redaction/masking: same method; e.g. `A12345678` -> `****5678` (`'****'+digits[-4:]`), only if the instructions ask.

## Fill blanks <!-- src: pdf-editing#common-pattern-fill-empty-form-fields -->
Find the label with `search_for`, insert value at `(label.x1+5, label.y1)` fontsize 11, so it sits ON the line/inside the box, not overlapping other text. Dates: format as in the document/instructions. Signature: the person's full name. Check for real AcroForm widgets (`page.widgets()`) and set `widget.field_value` + `widget.update()` if present.

## Verify and save
Save to `/root/output/output.pdf` (create the dir; `doc.save(path)` not same file as input). Re-open: text extractable, old values gone, new values present once, labels intact. Render to PNG only for viewing, never as output.

## Record
After each deliverable, run the taste context's `made` command with its defining choices.
