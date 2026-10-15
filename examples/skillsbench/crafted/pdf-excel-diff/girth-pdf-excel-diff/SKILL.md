---
name: girth-pdf-excel-diff
description: Use when comparing an old PDF table backup against a current Excel file to report deleted and modified records as JSON.
metadata:
  girth:
    sources:
      - path-or-repo: pdf
        sections: [pdfplumber - Text and Table Extraction, Extract Tables, Advanced Table Extraction]
      - path-or-repo: xlsx
        sections: [Editing existing Excel files, All Excel files]
    crafted_for: Extract employee table from old PDF, compare with current Excel, write diff_report.json of deleted and modified employees
    created: 2026-10-15
    last_used: 2026-10-15
---
## Extract PDF tables <!-- src: pdf#pdfplumber---text-and-table-extraction, pdf#extract-tables, pdf#advanced-table-extraction -->
```python
import pdfplumber, pandas as pd
rows = []
with pdfplumber.open("old.pdf") as pdf:
    for page in pdf.pages:
        for table in page.extract_tables():
            if table:
                rows.extend(table)
```
- Tables may span pages: the header row can repeat on each page, so drop repeated header rows.
- Build `pd.DataFrame(rows[1:], columns=rows[0])`. PDF cells are all strings: strip whitespace, convert numeric fields (e.g. Salary, Years, Score) to numbers.
- If `extract_tables()` returns nothing, fall back to `page.extract_text()` and split lines.

## Read the Excel file <!-- src: xlsx#all-excel-files, xlsx#editing-existing-excel-files -->
- Use `pandas.read_excel(path)` or `openpyxl.load_workbook(path)`; check `wb.sheetnames`. Read only; never modify the source files.
- Normalise both sides identically (strip strings, same dtypes, IDs as text like `EMP00002`) before comparing.

## Diff and report
- Index both by ID. Deleted = IDs in PDF but not in Excel. Ignore IDs only in Excel (additions) unless asked.
- Modified = for IDs in both, every column whose value differs: one entry per changed field `{id, field, old_value, new_value}`; old from PDF, new from Excel.
- Numeric fields as JSON numbers (int when whole, not "50020"; no numpy types, cast with `int()`/`float()`); text as strings.
- Sort `deleted_employees` and `modified_employees` by ID.
- Write with `json.dump(report, f, indent=2)` to the requested path; re-read it to verify.

## Record
After each deliverable, run the taste context's `made` command with its defining choices.
