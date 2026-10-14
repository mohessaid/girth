# Security Policy

## Supported versions

| Version | Supported |
|---|---|
| latest release | yes |
| older releases | no |

## Reporting a vulnerability

Please **do not** open a public issue. Report vulnerabilities privately through GitHub's private vulnerability reporting:

**https://github.com/mohessaid/girth/security/advisories/new**

What to expect:
- **Within 3 days:** an acknowledgement.
- **Within 14 days:** an initial assessment and severity rating.
- **Within 90 days:** a fix or mitigation, released with an advisory that credits you (unless you prefer otherwise).
- **Coordinated disclosure:** please give us that window before publishing details.

## Scope and security model

- The `girth` binary **reads** skill files. It never executes them.
- The skill launcher downloads release binaries over HTTPS from this repository's GitHub Releases. It **refuses to run a binary whose SHA-256 doesn't match the release's `SHA256SUMS`**.
- Release binaries are signed with Sigstore (keyless, `*.sigstore.json` bundles) and carry GitHub build-provenance attestations. To verify one: `gh attestation verify girth-<target> --repo mohessaid/girth`.
- Crafted skills are distilled from third-party skill content. Treat that content as untrusted input, and report any way it can cause code execution or escape the project's skill directory.
- The frontmatter parser and validator are continuously fuzzed (`fuzz/`, `.github/workflows/fuzz.yml`).
