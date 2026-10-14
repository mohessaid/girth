# Security Policy

## Supported versions

Only the latest release receives fixes.

## Reporting a vulnerability

Please **do not** open a public issue. Use GitHub's private reporting instead: **Security → Report a vulnerability** on this repository. You'll get an acknowledgement within a few days.

## Scope notes

- The `girth` binary only reads skill files; it never executes them.
- The skill launcher downloads release binaries over HTTPS from this repository's GitHub Releases and **refuses to run one whose SHA-256 doesn't match the release's `SHA256SUMS`**.
- Crafted skills are distilled from third-party skill content. Treat that content as untrusted input, and report any way it can cause code execution.
