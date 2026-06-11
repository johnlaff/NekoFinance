# Security Policy

## Reporting a vulnerability

Please report vulnerabilities privately via
[GitHub Security Advisories](https://github.com/johnlaff/NekoFinance/security/advisories/new)
— do not open a public issue for security problems.

You should expect an initial response within a week. As a local-first desktop app, the most
sensitive surfaces are the OAuth token store, the Google Sheets connector, and the SQLite
database in the user's app-data directory.

## Supported versions

Only the latest release receives fixes.

## Verifying releases

All release artifacts carry SLSA build provenance. Verify any downloaded binary with:

```bash
gh attestation verify <file> --repo johnlaff/NekoFinance
```
