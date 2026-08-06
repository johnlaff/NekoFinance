# AGENTS.md — Rust shell and functional core

Scope: everything under `src-tauri/`. The root `AGENTS.md` still applies; this file adds the Rust-specific contract.

## Layout

- `src/reading/` — `ForecastInputs`/`load_inputs` (the route's only SQL boundary) and the pure `compose` that yields `ForecastReading`; every DTO, dashboard summary, scenario, and Mia state tool is a slice of that one reading.
- `src/forecast/` — the engine's rulers (annual ruler, guardrail windows, savings floor). Different windows must never mean different derivations (ADR-0005).
- `src/commands/` — Tauri command modules; CRUD lives beside, not inside, projection routes (e.g. `budget_cmds.rs` vs `forecast_cmds.rs`).
- `src/mia/` — copilot facade, tools, bench, and provider pins.

## Rules

- Edition 2024. Gates: `npm run rust:check` (= `cargo check --locked` + `rustfmt --check` + `clippy -D warnings` + `cargo test --all-targets --all-features`). Single test: `cargo test <name> --manifest-path src-tauri/Cargo.toml`.
- Finance math is deterministic and tested; monetary values in integer cents, percentages in basis points, display truncation happens at the edge.
- SQLite pool has a single connection in production: never read through the pool while a write transaction is open — derive before `begin` or read via `&mut *tx`. Regression tests for this class must use a pool of 1 (in-memory defaults hide the deadlock).
- Parse and validate at boundaries; nothing from Sheets rows, local packs, or LLM output is trusted past the adapter.
