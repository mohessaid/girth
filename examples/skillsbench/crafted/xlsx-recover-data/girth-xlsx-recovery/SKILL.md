---
name: girth-xlsx-recovery
description: Use when an Excel workbook has missing cells (e.g. "???") that must be recovered from totals, shares, YoY changes, CAGR or cross-sheet relationships and saved as a new xlsx.
metadata:
  girth:
    sources:
      - path-or-repo: skillsbench/tasks/xlsx-recover-data/environment/skills/data-reconciliation
        sections: [Core Principles, Recovery Strategy, Chain Dependencies, Common Patterns in Budget Data]
      - path-or-repo: skillsbench/tasks/xlsx-recover-data/environment/skills/xlsx
        sections: [Preserve Existing Templates (when updating templates), Editing existing Excel files, Working with openpyxl]
    crafted_for: Recover "???" values in nasa_budget_incomplete.xlsx using cross-sheet relationships and save nasa_budget_recovered.xlsx
    created: 2026-10-15
    last_used: 2026-10-15
---
## Recovery formulas <!-- src: data-reconciliation#core-principles -->
- Sum: `Missing = Total - Σ(known)` (rows and columns)
- YoY forward: `Current = Previous × (1 + YoY/100)`; backward: `Previous = Current / (1 + YoY/100)`
- Share: `Part = Total × share/100`; inverse `Total = Part / (share/100)`
- CAGR: `((End/Start)^(1/years) - 1) × 100`; `End = Start × (1+CAGR/100)^years`; `Start = End / (1+CAGR/100)^years`

## Strategy <!-- src: data-reconciliation#recovery-strategy, data-reconciliation#chain-dependencies -->
1. Identify constraints between sheets (totals, shares, YoY, CAGR).
2. Find cells solvable now; solve in dependency order (a value recovered in one sheet may unlock a YoY/share cell in another). Map dependencies before starting; iterate until no "???" remain.
3. Validate: row/column totals match, shares consistent, YoY recalculates correctly.

## Patterns <!-- src: data-reconciliation#common-patterns-in-budget-data -->
| Constraint | Formula | Use when |
|---|---|---|
| Sum to total | Total - Σ(known) | one component missing |
| YoY forward/backward | Old×(1+%/100) / New/(1+%/100) | know one year + change |
| Share | Total × share% | know total + % |
| CAGR | ((End/Start)^(1/n)-1)×100 | multi-year growth |

## Editing the workbook <!-- src: xlsx#preserve-existing-templates-when-updating-templates, xlsx#editing-existing-excel-files, xlsx#working-with-openpyxl -->
- Match existing format and conventions exactly; never restyle.
- Use `openpyxl.load_workbook(path)` (cells 1-based), loop `wb.sheetnames`, write with `wb.save(new_path)`.
- Never open with `data_only=True` and save: formulas are permanently lost. Use it only to read values.
- Replace each "???" with a computed numeric value (not a string); save under the requested new filename, leaving the input untouched.
- After saving, reload and confirm no "???" remain in any sheet.

## Record
After each deliverable, run the taste context's `made` command with its defining choices.
