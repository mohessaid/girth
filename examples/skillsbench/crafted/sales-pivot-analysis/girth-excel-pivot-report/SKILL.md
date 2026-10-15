---
name: girth-excel-pivot-report
description: Use when building an xlsx report with openpyxl pivot tables and a SourceData sheet, from a PDF table joined with an Excel file (pdfplumber + pandas).
metadata:
  girth:
    sources:
      - path-or-repo: skills/xlsx
        sections: [Creating Pivot Tables, Basic Pivot Table Structure, Common Pivot Table Configurations, Pivot Table Field Configuration Reference, Reading Data]
      - path-or-repo: skills/pdf
        sections: [Table Extraction with pdfplumber, Extract All Tables from Multi-Page PDF, Clean Extracted Table Data]
    crafted_for: Build demographic_analysis.xlsx with 4 pivot sheets plus SourceData from population.pdf and income.xlsx
    created: 2026-10-15
    last_used: 2026-10-15
---
## Read the PDF <!-- src: pdf#table-extraction-with-pdfplumber, pdf#extract-all-tables-from-multi-page-pdf, pdf#clean-extracted-table-data -->
Never use the Read tool on PDFs; use pdfplumber. Always iterate ALL pages (continuation pages may lack headers).
```python
with pdfplumber.open(p) as pdf:
    for page in pdf.pages:
        for t in page.extract_tables():
            if t and len(t) > 1: dfs.append(pd.DataFrame(t[1:], columns=t[0]))
df = pd.concat(dfs, ignore_index=True)
df.columns = df.columns.str.strip(); df = df.dropna(how='all')
```
Drop repeated header rows; `pd.to_numeric` numeric columns.

## Read Excel <!-- src: xlsx#reading-data -->
`pd.read_excel('f.xlsx')`; `sheet_name=None` gives all sheets as dict. Merge with the PDF data on the shared key (e.g. SA2 code); check row counts after the merge.

## Enrichment (own notes)
- Quarter from MEDIAN_INCOME quartiles over ALL regions: `pd.qcut(df.MEDIAN_INCOME, 4, labels=["Q1","Q2","Q3","Q4"])`; store as plain strings.
- Total = EARNERS * MEDIAN_INCOME. SourceData: header in row 1, all original columns + Quarter + Total.
- Sheet names exactly as requested; one pivot per sheet.

## Pivot tables <!-- src: xlsx#creating-pivot-tables, xlsx#basic-pivot-table-structure -->
openpyxl: `from openpyxl.pivot.table import TableDefinition, Location, PivotField, DataField, RowColField`; `from openpyxl.pivot.cache import CacheDefinition, CacheField, CacheSource, WorksheetSource, SharedItems`.
**CRITICAL: every pivot MUST use `cacheId=0`**, else openpyxl raises KeyError on reload.
- `CacheDefinition(cacheSource=CacheSource(type="worksheet", worksheetSource=WorksheetSource(ref="A1:<lastcol><n>", sheet="SourceData")), cacheFields=[CacheField(name=col, sharedItems=SharedItems()) ...])`, one per source column, in column order.
- `TableDefinition(name=..., cacheId=0, dataCaption="Values", location=Location(ref="A3:B10", firstHeaderRow=1, firstDataRow=1, firstDataCol=1))`.
- One `PivotField` per source column, in order; `pivot.pivotFields[i]` set to `PivotField(axis="axisRow", showAll=False)`, `axis="axisCol"`, or `PivotField(dataField=True, showAll=False)`; others `PivotField(showAll=False)`.
- `pivot.rowFields.append(RowColField(x=i))`, `pivot.colFields.append(RowColField(x=j))`, `pivot.dataFields.append(DataField(name=..., fld=k, subtotal="sum"))`.
- Attach: `pivot.cache = cache; ws._pivots.append(pivot)`. Values compute on open; Location ref is approximate.

## Aggregations <!-- src: xlsx#common-pivot-table-configurations, xlsx#pivot-table-field-configuration-reference -->
subtotal: sum, count, average, max, min, product, stdDev, var. Count = `subtotal="count"` on any field (e.g. region/SA2 id). Rows x columns pivot: STATE `axisRow`, Quarter `axisCol`, EARNERS data/sum.

## Verify (own notes)
Reload with `load_workbook`; confirm 5 sheet names, one `_pivots` entry per pivot sheet, SourceData headers/rows. Cross-check with pandas groupby.

## Record
After each deliverable, run the taste context's `made` command with its defining choices.
