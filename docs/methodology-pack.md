# Methodology Pack

The methodology pack is private, source-neutral, and excluded from git. It converts private learning material into anonymized rules, workflows, examples, and evals.

## Allowed In Public Repo

- Generic schemas.
- Synthetic examples.
- Source-neutral rule IDs such as `method.reserve.001`.
- Public-safe architecture notes.

## Forbidden In Public Repo

- Source names, domains, course titles, lesson titles, quotes, transcripts, videos, attachments, screenshots, IDs, URLs, signed URLs, community posts, author names, emails, handles, embeddings, vector indexes, OAuth state, and personal financial data.

## Local Pack Shape

```text
.methodology-pack/
  manifest.json
  rules/*.json
  workflows/*.json
  chunks/*.jsonl
  evals/*.jsonl
  indexes/
```

## Minimum Manifest

```json
{
  "schemaVersion": 1,
  "name": "private-finance-methodology",
  "sourcePolicy": "anonymized-private-derived",
  "generatedAt": "2026-06-08T00:00:00.000Z",
  "rules": [],
  "workflows": [],
  "evals": []
}
```

## Loader Requirements

- Reject packs that contain source names, domains, URLs, emails, handles, or signed links.
- Validate every rule and workflow against a schema before indexing.
- Store vector/FTS indexes in local ignored directories only.
- Preserve traceability by neutral IDs, not by source references.
