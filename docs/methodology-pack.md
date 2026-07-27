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

The copilot reads the pack from `<app data dir>/methodology-pack`. Only three entries are part of
the runtime contract; anything else in the directory is curation material and is never served.

```text
methodology-pack/
  core.md            # method core, assembled into the stable system prompt prefix
  chapters/*.md      # one file per get_method_guidance topic
  forbidden*.txt     # deny-lists, one literal substring per line, `#` starts a comment
```

Topics are a closed vocabulary, and each one needs a file named after it in `chapters/`:
`metodo` · `diario` · `cartao` · `economia` · `reserva` · `dividas` · `financiamento` ·
`patrimonio` · `renda` · `casal` · `planejamento`.

## Loader Requirements

- Serve nothing without a deny-list: at least one `forbidden*.txt` in the pack root, carrying at
  least one effective pattern. A pack that cannot be scanned refuses instead of degrading.
- Scan at the moment of serving, over the content that actually leaves the machine — the chapter
  a tool returns, and the whole assembled prompt prefix, not only the core file it came from.
- Report the deny-list file and the entry number that matched, never the matched term. Name
  deny-list files with neutral names: the filename reaches the error message.
- Keep the assembled prefix within the token budget declared by the prompt module. A core over
  budget is an error to fix in curation, never a truncated prefix.
- A missing `core.md` degrades the conversation to numbers only; a `core.md` that exists and
  cannot be read is an error, because a broken pack must not look like an absent one.
