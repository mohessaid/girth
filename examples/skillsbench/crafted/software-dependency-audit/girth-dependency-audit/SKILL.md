---
name: girth-dependency-audit
description: Use when auditing package-lock.json or similar dependency files for HIGH/CRITICAL vulnerabilities offline with Trivy and writing a CSV report with CVSS scores.
metadata:
  girth:
    sources:
      - path-or-repo: skillsbench/software-dependency-audit/environment/skills/trivy-offline-vulnerability-scanning
        sections: [Trivy Database Structure, Offline Scanning Workflow, JSON Output Structure, Common Issues]
      - path-or-repo: skillsbench/software-dependency-audit/environment/skills/cvss-score-extraction
        sections: [Multiple Vulnerability Data Sources, Source Priority Strategy, Data Structure, Enhanced Version with V2 Fallback]
      - path-or-repo: skillsbench/software-dependency-audit/environment/skills/vulnerability-csv-reporting
        sections: [CSV Schema Design for Security Reports, Basic Usage, Implementation, Extracting Fields from Scanner Output]
    crafted_for: Audit /root/package-lock.json offline for HIGH/CRITICAL vulns and write /root/security_audit.csv
    created: 2026-10-15
    last_used: 2026-10-15
---
## Offline Trivy scan <!-- src: trivy-offline-vulnerability-scanning#offline-scanning-workflow, trivy-offline-vulnerability-scanning#trivy-database-structure -->
DB is `<cache-dir>/db/trivy.db` (+ `metadata.json`). Verify it exists before scanning; if missing, search the filesystem for `trivy.db` (e.g. `~/.cache/trivy`) and use that cache dir.

```
trivy fs <package-lock.json> --format json --output report.json --scanners vuln --skip-db-update --offline-scan --cache-dir <cache>
```
`--skip-db-update` is critical: never update online. Don't treat non-zero exit as success. Empty results despite known vulns = outdated DB. "failed to initialize DB" = wrong `--cache-dir` or corrupt DB. Add `--severity HIGH,CRITICAL` or filter afterwards.

## JSON fields <!-- src: trivy-offline-vulnerability-scanning#json-output-structure -->
`Results[].Vulnerabilities[]`: `VulnerabilityID`, `PkgName`, `InstalledVersion`, `FixedVersion` (may be absent), `Severity`, `Title`, `PrimaryURL`, `CVSS{nvd|ghsa|redhat:{V2Score,V3Score}}`. `Results` may be missing or have no `Vulnerabilities`; use `.get`.

## CVSS score <!-- src: cvss-score-extraction#source-priority-strategy, cvss-score-extraction#enhanced-version-with-v2-fallback -->
Priority cascade of V3Score: `nvd` → `ghsa` → `redhat`; then `nvd` V2Score; else `N/A`. Use `is not None` checks (0.0 is valid).

## CSV output <!-- src: vulnerability-csv-reporting#csv-schema-design-for-security-reports, vulnerability-csv-reporting#basic-usage -->
Columns exactly, in order: `Package,Version,CVE_ID,Severity,CVSS_Score,Fixed_Version,Title,Url`.
Use `csv.DictWriter(f, fieldnames=headers)` with `open(path,'w',newline='',encoding='utf-8')` and `writeheader()`.
Missing values are `N/A`, never empty (Fixed_Version especially). Keep only Severity in `HIGH`, `CRITICAL`. Default Url to `PrimaryURL`. Dedupe identical rows. Always write the file, header-only if no findings.

## Record
After each deliverable, run the taste context's `made` command with its defining choices.
