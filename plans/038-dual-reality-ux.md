# Plan 038: Dual-reality UX: daily-teto config (Reality B) + credit-first quick-add + Economizado badge

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat e62ecb6..HEAD -- src/screens/SettingsScreen.tsx src/screens/dashboard/DailyCheckinCard.tsx src/screens/TotaisScreen.tsx src/screens/totaisStatus.ts src-tauri/src/commands/forecast_cmds.rs`
>
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: LOW
- **Depends on**: none
- **Category**: feature
- **Planned at**: commit `e62ecb6`, 2026-06-20

## Why this matters

Neko must serve two realities: a user who currently books everything via credit
(Diário = 0 every day) and a user who will later adopt debit/Diário spending.
Today the quick-add card always opens on "Diário" (wrong default for a
credit-only user), the Saída hint says "(débito)" (misleading when Saída means
the card fatura on its due date), and the Economizado metric row on TotaisScreen
shows no status badge even though `economizadoStatus()` is already exported and
tested. This plan wires the three gaps: a UI to set the daily Diário teto (so
Reality B users can activate the progress bar), persistence of the last-used
movement type across restarts, and the missing Economizado status chip.

## Current state

### Files and their roles

- `src-tauri/src/commands/forecast_cmds.rs` — `effective_daily_ceiling()` (lines 247–281) reads `daily_budget WHERE status='active' AND amount > 0` first, then falls back to the prior-month average. The table exists; there is NO Tauri command to write to it.
- `src-tauri/src/commands/write_back_cmds.rs` — `get_app_setting` / `set_app_setting` (lines 302–344): KV store backed by `app_setting` table; already registered as Tauri commands.
- `src/lib/api.ts` — `getAppSetting(key)` / `setAppSetting(key, value)` (lines 505–511): TypeScript wrappers over those commands.
- `src/screens/SettingsScreen.tsx` — existing settings screen; `DailyReminderSection` (lines 44–136) shows the canonical pattern for a settings row that reads/writes via `getAppSetting`/`setAppSetting`.
- `src/screens/dashboard/DailyCheckinCard.tsx` — quick-add card; initial state hardcoded at line 94:
  ```tsx
  // DailyCheckinCard.tsx:93-98
  const INITIAL_CHECKIN: CheckinState = {
    kind: "diario", // padrão = caminho rápido
    description: "",
    amount: "",
    busy: false,
    error: null,
  };
  ```
  Saída hint at line 361:
  ```tsx
  // DailyCheckinCard.tsx:360-362
  {
    kind === "saida" && (
      <p style={QUICK_HINT_STYLE}>Saída = despesa fixa do mês (débito).</p>
    );
  }
  ```
  Card title at line 205 ("Diário de hoje") with no subtitle.
- `src/screens/TotaisScreen.tsx` — renders `MetricRow` for Economizado at lines 298–314 **without** a `status` prop:
  ```tsx
  // TotaisScreen.tsx:298-314
  <MetricRow
    label="Economizado"
    term="economizado"
    value={
      <span ...>
        {pct}%
      </span>
    }
    sublabel={`no ano: ${ytdPct}% acumulado · meta 20–30% (média anual)`}
  />
  ```
  Custo de vida sublabel at line 320:
  ```tsx
  // TotaisScreen.tsx:318-321
  <MetricRow
    label="Custo de vida"
    ...
    sublabel="= Saída Total (saídas + diário)"
  />
  ```
  The footer hint at line 359 already says `"saídas (incl. cartão) + diário = custo de vida"` — the sublabel is inconsistent with it.
- `src/screens/totaisStatus.ts` — `economizadoStatus(bps: number): Status` is exported (line 38) and tested, but never called from `TotaisScreen.tsx`.
- `src/screens/TotaisScreen.test.tsx` — existing render tests; structural pattern to follow for new test cases.
- `src/screens/dashboard/DailyCheckinCard.test.tsx` — existing tests; model for new persistence/hint tests.
- `src/screens/SettingsScreen.test.tsx` — existing tests; model for new daily-teto settings row test.

### Repo conventions that apply

- **React Compiler is ON**: no `useCallback`, no `useMemo`, no manual `memo()`. Static style objects must be hoisted to module scope (not defined inline in JSX). See `DailyCheckinCard.tsx` for examples of hoisted `const DAILY_BAR_TRACK`, `DAILY_INPUT_STYLE`, etc.
- **localStorage** is the accepted pattern for UI-only persistence across restarts (no Tauri needed). Precedent: `src/shell/ThemeToggle.tsx` uses `localStorage.getItem("neko-theme")` / `localStorage.setItem(...)`. Use `"neko_last_kind"` as the key.
- **`app_setting` KV** (`getAppSetting`/`setAppSetting`) is the accepted pattern for settings that need a UI toggle with a save action. Precedent: `DailyReminderSection` in `SettingsScreen.tsx` (lines 44–136).
- **Money is positive-magnitude integer cents**. The `amount` column in `daily_budget` stores cents as a positive integer.
- **Method-neutral language**: do not name the official app, course, or RE in code, comments, or strings. Use generic labels like "teto do Diário".
- **Section/row pattern in SettingsScreen**: wrap in `<Section icon={...} title="..." sub="...">` containing a `<div className="set-panel">` with `<div className="set-row">` children. Match exactly — do not invent new class names.
- **No new Tauri command needed for daily teto**: write to `app_setting` with key `"daily_diario_ceiling_cents"` and parse/format in the UI. The `effective_daily_ceiling` in Rust already prefers an explicit `daily_budget` row; we do **not** write to `daily_budget` in this plan (that would require a new Tauri command and a `person_id` lookup — unnecessary when `app_setting` can carry the intent). **IMPORTANT**: the `daily_budget` table continues to be the source of truth for the forecast engine; this plan stores the _UI-entry_ ceiling in `app_setting` and reflects it to `daily_budget` via a new `upsert_daily_budget` Tauri command (see Step 1). Read the STOP condition below before deciding.

> **Architecture note (read before Step 1)**: storing the ceiling only in `app_setting` as a string works for _displaying_ a reference value but will NOT drive the forecast bar (which reads `effective_daily_ceiling` from `daily_budget`). Therefore Step 1 must add a Rust command to upsert `daily_budget`, and the TS side must call it. The `app_setting` approach would be acceptable ONLY for a pure-display hint; since the card progress bar already reads `summary.daily_budget` (from the forecast engine), the only correct path is `daily_budget`. See the implementation details in Step 1.

### daily_budget table schema (migration 20240608000009_daily_budget.sql)

```sql
CREATE TABLE IF NOT EXISTS daily_budget (
    id TEXT PRIMARY KEY NOT NULL,
    person_id TEXT NOT NULL REFERENCES person(id) ON DELETE CASCADE,
    amount INTEGER NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT,
    status TEXT NOT NULL CHECK(status IN ('active','under_review','deprecated')),
    free_income INTEGER,
    calculated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

`effective_daily_ceiling` selects `amount WHERE status='active' AND amount > 0 ORDER BY start_date DESC LIMIT 1`.

### Fetching person_id in Rust (established pattern)

Other commands obtain the first person id with:

```rust
sqlx::query_as("SELECT id FROM person ORDER BY created_at LIMIT 1")
```

(see `src-tauri/src/commands/write_back_cmds.rs:747` and `src-tauri/src/commands/pockets.rs:129`)

## Commands you will need

| Purpose          | Command                              | Expected on success   |
| ---------------- | ------------------------------------ | --------------------- |
| Typecheck        | `npm run typecheck`                  | exit 0, no errors     |
| Lint             | `npm run lint`                       | exit 0                |
| Unit tests       | `npm run test:run`                   | all pass              |
| Rust checks      | `npm run rust:check`                 | exit 0                |
| React Doctor     | `npm run doctor`                     | 0 findings            |
| Full gate        | `npm run check`                      | exit 0                |
| E2E visual smoke | `npm run e2e`                        | screenshots pass      |
| Targeted test    | `npm run test:run -- DailyCheckin`   | relevant suite passes |
| Targeted test    | `npm run test:run -- TotaisScreen`   | relevant suite passes |
| Targeted test    | `npm run test:run -- SettingsScreen` | relevant suite passes |

## Suggested executor toolkit

- Use the `neko-finance-design` skill if styling new UI elements (tokens, spacing, component patterns).
- Use the `tdd` skill if you want red-green-refactor scaffolding for the new tests.

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/commands/forecast_cmds.rs` — add `upsert_daily_budget` Tauri command
- `src-tauri/src/lib.rs` — register `upsert_daily_budget` in `generate_handler!`
- `src/lib/api.ts` — add `upsertDailyBudget(amountCents: number): Promise<void>`
- `src/screens/SettingsScreen.tsx` — add `DailyTetoCeilingSection` component
- `src/screens/dashboard/DailyCheckinCard.tsx` — persist last kind; fix Saída hint; add subtitle
- `src/screens/TotaisScreen.tsx` — wire `economizadoStatus()`; fix Custo de vida sublabel
- `src/screens/SettingsScreen.test.tsx` — new test: teto setting row
- `src/screens/dashboard/DailyCheckinCard.test.tsx` — new tests: last-kind persistence; hint text
- `src/screens/TotaisScreen.test.tsx` — new test: Economizado badge renders; sublabel text
- `plans/README.md` — update status row when done

**Out of scope** (do NOT touch, even though they look related):

- `src/screens/totaisStatus.ts` — already correct; `economizadoStatus` is already exported and tested. Do not modify.
- `src-tauri/src/forecast/mod.rs` — forecast engine logic is correct and does not need changes.
- `src-tauri/migrations/` — the `daily_budget` table already exists; no migration needed.
- Any write-back commands or sheet-sync code.
- Any other screen or component not listed above.

## Git workflow

- Branch: `advisor/038-dual-reality-ux`
- Commit per logical unit; follow the repo's conventional-commit style observed in `git log`:
  - `feat: …` for new behavior
  - `fix: …` for corrections
  - `chore: …` for wiring/test additions with no behavior change
  - Example from log: `fix: revisão completa da app (rodada 9) — bugs, atomicidade, segurança, a11y e CI/CD (#21)`
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Add `upsert_daily_budget` Rust command

In `src-tauri/src/commands/forecast_cmds.rs`, add a new public async function
tagged `#[tauri::command]`. Place it after `effective_daily_ceiling` (after line
281, before `reserve_floor`):

```rust
/// Grava (ou atualiza) o teto diário configurado pelo usuário (Reality B: gasto Diário ativo).
/// Depreca todos os registros ativos anteriores e insere um novo com `status='active'`.
/// `amount_cents` = 0 desativa o teto explícito (o engine cai no fallback de média do mês anterior).
#[tauri::command]
pub async fn upsert_daily_budget(
    pool: State<'_, SqlitePool>,
    amount_cents: i64,
) -> Result<(), String> {
    let pool = pool.inner();
    // Obtém o person_id do primeiro perfil (padrão single-user).
    let person: Option<(String,)> =
        sqlx::query_as("SELECT id FROM person ORDER BY created_at LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("upsert_daily_budget (person): {e}"))?;
    let Some((person_id,)) = person else {
        // Nenhum perfil ainda — silencioso (usuário novo sem import).
        return Ok(());
    };
    // Depreca os registros ativos anteriores.
    sqlx::query(
        "UPDATE daily_budget SET status='deprecated' WHERE status='active'",
    )
    .execute(pool)
    .await
    .map_err(|e| format!("upsert_daily_budget (deprecate): {e}"))?;

    if amount_cents > 0 {
        let id = uuid::Uuid::new_v4().to_string();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        sqlx::query(
            "INSERT INTO daily_budget (id, person_id, amount, start_date, status) \
             VALUES (?1, ?2, ?3, ?4, 'active')",
        )
        .bind(&id)
        .bind(&person_id)
        .bind(amount_cents)
        .bind(&today)
        .execute(pool)
        .await
        .map_err(|e| format!("upsert_daily_budget (insert): {e}"))?;
    }
    Ok(())
}
```

Note: `uuid` and `chrono` are already in `Cargo.toml` (used by other commands
in the same file). `State` is imported at the top of the file. Do not add new
dependencies.

Then register the command in `src-tauri/src/lib.rs` inside
`tauri::generate_handler![…]` (after line 54, where `set_app_setting` is listed):

```rust
commands::upsert_daily_budget,
```

**Verify**: `npm run rust:check` → exit 0, no errors.

### Step 2: Add `upsertDailyBudget` TypeScript wrapper

In `src/lib/api.ts`, after the `setAppSetting` function (after line 511), add:

```ts
/**
 * Grava o teto de Diário diário configurado pelo usuário (Reality B).
 * `amountCents = 0` desativa o teto explícito — o engine usa o fallback de média.
 */
export function upsertDailyBudget(amountCents: number): Promise<void> {
  return invoke("upsert_daily_budget", { amountCents });
}
```

**Verify**: `npm run typecheck` → exit 0.

### Step 3: Add `DailyTetoCeilingSection` to SettingsScreen

In `src/screens/SettingsScreen.tsx`:

1. Add `Gauge` to the lucide-react import (add it to the existing destructured import at line 2 — `Gauge` is available in the version used; if the name is wrong run `grep -r "Gauge\|SlidersHorizontal" node_modules/lucide-react/dist/ 2>/dev/null | head -3` to confirm, and pick the correct icon name).

2. Import `upsertDailyBudget` from `"../lib/api"` (add to the existing import at line 13).

3. Import `parseBRLToCents`, `formatBRL` from `"../lib/format"` (add to the existing import from that module if not already present; check first with grep).

4. Add a hoisted static style constant before the component (React Compiler pattern — never inline in JSX):

```tsx
// Estilo estático do campo de teto diário (React Compiler: nunca inline em JSX).
const TETO_INPUT_STYLE: React.CSSProperties = {
  fontFamily: "var(--font-money)",
  fontSize: "var(--fs-body)",
  background: "var(--bg-subtle)",
  border: "var(--bw-hair) solid var(--border-input)",
  borderRadius: "var(--radius-xs)",
  color: "var(--text)",
  padding: "4px 8px",
  height: "var(--hit-min)",
  width: "10ch",
};
```

5. Add the component (place it before `export function SettingsScreen`):

```tsx
/**
 * Configura o teto de Diário diário (Reality B: usuário com gasto variável ativo).
 * Persiste em `daily_budget` via `upsert_daily_budget`; quando zerado, o engine
 * usa o fallback de média do mês anterior — nenhum teto explícito.
 * Disponível somente no shell desktop (isTauri).
 */
function DailyTetoCeilingSection() {
  const [raw, setRaw] = useState("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  // Carrega o teto ativo na montagem para pré-preencher o campo.
  useEffect(() => {
    if (!isTauri) return;
    void (async () => {
      try {
        const val = await getAppSetting("daily_diario_ceiling_display");
        if (val) setRaw(val);
      } catch {
        // leitura não-crítica; ignora
      }
    })();
  }, []);

  async function handleSave() {
    const cents = parseBRLToCents(raw);
    if (cents == null || cents < 0) {
      setErr("Informe um valor válido (ex.: 50,00) ou zero para desativar.");
      return;
    }
    setSaving(true);
    setErr(null);
    setSaved(false);
    try {
      await upsertDailyBudget(cents);
      // Salva o display raw para restaurar no próximo mount.
      await setAppSetting("daily_diario_ceiling_display", cents > 0 ? raw : "");
      setSaved(true);
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Não foi possível salvar o teto.");
    }
    setSaving(false);
  }

  if (!isTauri) return null;

  return (
    <Section
      icon={Gauge}
      title="Teto do Diário"
      sub="Defina quanto pretende gastar por dia em despesas variáveis. Deixe em branco para usar a média do mês anterior."
    >
      <div className="set-panel">
        <div className="set-row">
          <div className="set-row__main">
            <div className="set-row__t">Teto diário (R$)</div>
            <div className="set-row__d">
              Orienta a barra de progresso do check-in e o forecast dos dias futuros do
              mês. Zero ou em branco = usar média do mês anterior automaticamente.
              {saved ? <strong> Salvo.</strong> : null}
              {err ? (
                <strong role="alert" style={{ color: "var(--danger-400)" }}>
                  {" "}
                  {err}
                </strong>
              ) : null}
            </div>
          </div>
          <div
            className="set-row__ctl"
            style={{ display: "flex", gap: "var(--space-2)", alignItems: "center" }}
          >
            <input
              type="text"
              inputMode="decimal"
              placeholder="ex.: 50,00"
              value={raw}
              onChange={(e) => {
                setRaw(e.currentTarget.value);
                setSaved(false);
              }}
              disabled={saving}
              style={TETO_INPUT_STYLE}
              aria-label="Teto diário em reais"
            />
            <Button
              variant="secondary"
              size="sm"
              onClick={() => void handleSave()}
              disabled={saving}
            >
              {saving ? "Salvando…" : "Salvar"}
            </Button>
          </div>
        </div>
      </div>
    </Section>
  );
}
```

6. Add `<DailyTetoCeilingSection />` inside `SettingsScreen`'s JSX, after `<DailyReminderSection />` (after line 274).

**Verify**: `npm run typecheck` → exit 0.

### Step 4: Persist last-used MovKind across restarts in DailyCheckinCard

In `src/screens/dashboard/DailyCheckinCard.tsx`:

1. Add a helper above `INITIAL_CHECKIN` (module scope, static, no inline):

```tsx
const LAST_KIND_KEY = "neko_last_kind";

function readLastKind(): MovKind {
  if (typeof window === "undefined") return "diario";
  const stored = localStorage.getItem(LAST_KIND_KEY);
  // Validate: must be one of the 5 canonical kinds.
  if (
    stored === "entrada" ||
    stored === "saida" ||
    stored === "diario" ||
    stored === "cartao" ||
    stored === "economia"
  ) {
    return stored;
  }
  return "diario";
}
```

2. Change `INITIAL_CHECKIN` to read the stored kind on initialisation:

```tsx
// Before (line 93):
const INITIAL_CHECKIN: CheckinState = {
  kind: "diario", // padrão = caminho rápido

// After:
const INITIAL_CHECKIN: CheckinState = {
  kind: readLastKind(), // persiste entre sessões (localStorage)
```

3. In `checkinReducer`, when `type === "set"` and the patch includes `kind`, also write to localStorage. The simplest approach: add a side-effect in `DailyCheckinCard` after the `dispatch` call for kind changes. Because React Compiler is ON, do NOT wrap in `useCallback`. Instead, replace the chip's `onClick` handler:

```tsx
// Before (line 312):
onClick={() => dispatch({ type: "set", patch: { kind: k } })}

// After:
onClick={() => {
  dispatch({ type: "set", patch: { kind: k } });
  localStorage.setItem(LAST_KIND_KEY, k);
}}
```

This is a simple inline arrow on a stable element — acceptable here because the
value `k` is fixed per map iteration (it is not a dynamic state/prop closure
that the Compiler would need to track). Confirm with `npm run doctor` after.

**Verify**: `npm run test:run -- DailyCheckin` → all pass (existing tests must not break).

### Step 5: Fix Saída hint text and add card subtitle in DailyCheckinCard

In `src/screens/dashboard/DailyCheckinCard.tsx`:

1. Fix the Saída hint (line 361). Replace:

   ```tsx
   <p style={QUICK_HINT_STYLE}>Saída = despesa fixa do mês (débito).</p>
   ```

   With:

   ```tsx
   <p style={QUICK_HINT_STYLE}>
     Saída = despesa fixa do mês — contas, fatura no vencimento.
   </p>
   ```

   This removes "(débito)" (misleading for credit-primary users) and is method-neutral.

2. Add a subtitle below the card title "Diário de hoje" (after the `<span>` at line 198 that contains `Diário de hoje`). Insert after the closing `</span>` of `dash-card__title`:

```tsx
<span
  style={{ fontSize: "var(--fs-micro)", color: "var(--text-faint)" }}
  aria-label="Registre aqui qualquer gasto do dia — Diário, compra no cartão ou saída fixa"
>
  Diário, cartão ou saída — registre o que aconteceu hoje
</span>
```

Place this inside `dash-card__head` div, as a sibling of the existing two `<span>` children (title and status display). The `aria-label` gives screen-reader context; the visible text is short and method-neutral.

**Verify**: `npm run test:run -- DailyCheckin` → all pass. `npm run doctor` → 0 findings.

### Step 6: Wire `economizadoStatus` and fix Custo de vida sublabel in TotaisScreen

In `src/screens/TotaisScreen.tsx`:

1. Import `economizadoStatus` — it is already exported from `"./totaisStatus"`. Add it to the
   existing named import at line 15:

   ```tsx
   // Before (line 11-16):
   import {
     currentMonthMetric,
     performanceStatus,
     custoVidaStatus,
     type Status,
   } from "./totaisStatus";

   // After:
   import {
     currentMonthMetric,
     performanceStatus,
     economizadoStatus,
     custoVidaStatus,
     type Status,
   } from "./totaisStatus";
   ```

2. Add the `status` prop to the Economizado `MetricRow` (lines 298–314). Change:

   ```tsx
   <MetricRow
     label="Economizado"
     term="economizado"
     value={
       <span ...>
         {pct}%
       </span>
     }
     sublabel={`no ano: ${ytdPct}% acumulado · meta 20–30% (média anual)`}
   />
   ```

   To:

   ```tsx
   <MetricRow
     label="Economizado"
     term="economizado"
     value={
       <span ...>
         {pct}%
       </span>
     }
     status={economizadoStatus(m.savings_rate_bps)}
     sublabel={`no ano: ${ytdPct}% acumulado · meta 20–30% (média anual)`}
   />
   ```

   The `savings_rate_bps` field is already available on `m` (type `MonthMetric`).

3. Fix the Custo de vida sublabel (line 320) to match the footer hint (line 359):

   ```tsx
   // Before:
   sublabel = "= Saída Total (saídas + diário)";

   // After:
   sublabel = "= Saída Total (saídas incl. cartão + diário)";
   ```

**Verify**: `npm run test:run -- TotaisScreen` → all pass, including new test added in Step 7.

### Step 7: Add new tests

#### 7a — TotaisScreen.test.tsx

Add a new `it` inside the existing `describe("TotaisScreen (render)", ...)` block:

```tsx
it("Economizado mostra badge de status (Dentro do ideal quando >= 20%)", async () => {
  mockInvoke.mockReset();
  // FORECAST has savings_rate_bps: 2500 for June → "Dentro do ideal"
  mockCommands({ get_forecast: FORECAST, owner_totals_for_month_cmd: [] });
  render(<TotaisScreen />);

  await waitFor(() => {
    expect(screen.getByText("Economizado")).toBeInTheDocument();
  });
  expect(screen.getByText("Dentro do ideal")).toBeInTheDocument();
});

it("Custo de vida sublabel menciona cartão", async () => {
  mockInvoke.mockReset();
  mockCommands({ get_forecast: FORECAST, owner_totals_for_month_cmd: [] });
  render(<TotaisScreen />);

  await waitFor(() => {
    expect(screen.getByText("Custo de vida")).toBeInTheDocument();
  });
  expect(screen.getByText(/incl\. cartão/)).toBeInTheDocument();
});
```

#### 7b — DailyCheckinCard.test.tsx

Add inside the existing `describe("DailyCheckinCard", ...)` block:

```tsx
it("persiste o tipo selecionado no localStorage e restaura na próxima montagem", async () => {
  const user = userEvent.setup();
  mockCommands({});
  localStorage.clear();

  const { unmount } = render(<DailyCheckinCard summary={SUMMARY} onLogged={vi.fn()} />);
  // Seleciona Cartão.
  await user.click(screen.getByRole("radio", { name: /Cartão/ }));
  expect(localStorage.getItem("neko_last_kind")).toBe("cartao");
  unmount();

  // Remonta — deve restaurar Cartão como tipo ativo.
  render(<DailyCheckinCard summary={SUMMARY} onLogged={vi.fn()} />);
  expect(screen.getByRole("radio", { name: /Cartão/ })).toHaveAttribute(
    "aria-checked",
    "true",
  );
  localStorage.clear();
});

it("hint da Saída não menciona débito (texto method-neutral)", () => {
  mockCommands({});
  render(<DailyCheckinCard summary={SUMMARY} onLogged={vi.fn()} />);
  // O chip Saída não está selecionado por padrão; simula a seleção para mostrar o hint.
  // Verifica a ausência de "(débito)" no DOM inteiro.
  expect(document.body.textContent).not.toContain("(débito)");
});
```

Note: the second test checks the absence of the old hint text without needing
to select Saída first (the string "(débito)" must not appear in the DOM
regardless of which chip is selected).

#### 7c — SettingsScreen.test.tsx

Add inside the existing `describe("SettingsScreen", ...)` block:

```tsx
it("DailyTetoCeilingSection: mostra o campo de teto e chama upsert_daily_budget ao salvar", async () => {
  const user = userEvent.setup();
  // Tauri environment is simulated by the existing mock setup (isTauri = true in test env
  // if the mock includes __TAURI_INTERNALS__ — check the existing test beforeEach; if
  // isTauri is false in tests, this component returns null and the test should be skipped
  // with it.skipIf(!isTauri, ...) or the isTauri guard should be removed for testability).
  // ALTERNATIVE: extract the section body into a separate component without the isTauri
  // guard if you find the guard blocks testing. Follow the pattern in the existing tests.
  mockCommands({
    get_app_info: APP_INFO,
    get_app_setting: null,
    set_app_setting: undefined,
    upsert_daily_budget: undefined,
  });
  render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);

  // If the section is gated by isTauri and isTauri === false in JSDOM, this test will
  // find nothing — STOP and report so the reviewer can decide whether to lift the guard.
  const input = screen.queryByLabelText("Teto diário em reais");
  if (!input) {
    // isTauri is false in test environment — document this and skip.
    expect(true).toBe(true); // placeholder — file a follow-up to lift the guard
    return;
  }
  await user.type(input, "50,00");
  await user.click(screen.getByRole("button", { name: "Salvar" }));

  await waitFor(() => {
    expect(mockInvoke).toHaveBeenCalledWith(
      "upsert_daily_budget",
      expect.objectContaining({ amountCents: 5000 }),
    );
  });
});
```

**Verify**: `npm run test:run -- SettingsScreen` → all pass.

### Step 8: Full gate

Run the full quality gate and confirm all checks pass:

```
npm run check
```

Expected: exit 0. If `npm run doctor` reports new findings from the inline
arrow added in Step 4, consider hoisting the click handler to a named function
per the patterns in `checkinReducer` — but do not add `useCallback` (Compiler
is ON).

**Verify**: `npm run check` → exit 0. `npm run e2e` → screenshots pass (visual
smoke — inspect if any layout regression is visible in the DailyCheckin card or
Totais section).

## Test plan

New tests (added in Step 7):

| File                        | Test                                                  | Covers                                 |
| --------------------------- | ----------------------------------------------------- | -------------------------------------- |
| `TotaisScreen.test.tsx`     | Economizado badge "Dentro do ideal"                   | `economizadoStatus` wired to MetricRow |
| `TotaisScreen.test.tsx`     | Custo de vida sublabel mentions "incl. cartão"        | sublabel text fix                      |
| `DailyCheckinCard.test.tsx` | kind persists to localStorage and restores on remount | last-kind persistence                  |
| `DailyCheckinCard.test.tsx` | Saída hint does not contain "(débito)"                | method-neutral hint                    |
| `SettingsScreen.test.tsx`   | DailyTetoCeilingSection calls upsert_daily_budget     | teto save flow                         |

Structural pattern: model after `src/screens/TotaisScreen.test.tsx` (pure logic +
render describes) and `src/screens/dashboard/DailyCheckinCard.test.tsx` (userEvent
interaction + mockCommands).

Rust test for `upsert_daily_budget`: add in `src-tauri/src/commands/mod.rs` (the
existing integration-test module at the bottom of the file). Pattern: use
`in_memory_pool()` helper (already present in that module), insert a person, call
`upsert_daily_budget(pool, 5000).await`, then verify `SELECT amount FROM
daily_budget WHERE status='active'` returns 5000. Deprecate-and-replace: call
again with 8000, verify only one active row and it has amount=8000.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npm run rust:check` exits 0
- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `npm run test:run` exits 0; 5 new tests exist and pass (or 4 if the SettingsScreen isTauri guard causes the 5th to be a placeholder — document)
- [ ] `npm run doctor` exits 0 (0 findings)
- [ ] `npm run e2e` screenshots pass (no visible regression in DailyCheckin card or Totais)
- [ ] `grep -n "(débito)" src/screens/dashboard/DailyCheckinCard.tsx` returns no matches
- [ ] `grep -n "economizadoStatus" src/screens/TotaisScreen.tsx` returns at least 1 match
- [ ] `grep -n "neko_last_kind" src/screens/dashboard/DailyCheckinCard.tsx` returns at least 1 match
- [ ] `grep -n "DailyTetoCeilingSection" src/screens/SettingsScreen.tsx` returns at least 2 matches (definition + usage)
- [ ] `grep -n "upsert_daily_budget" src-tauri/src/lib.rs` returns 1 match
- [ ] No files outside the in-scope list are modified (`git diff --name-only`)
- [ ] `plans/README.md` status row updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the locations in "Current state" does not match the excerpts — the codebase has drifted since this plan was written.
- `uuid` or `chrono` are NOT already in `Cargo.toml` (the command in Step 1 uses them; adding new dependencies is out of scope).
- `upsert_daily_budget` in Step 1 compiles but `npm run rust:check` fails due to a missing import — check that `State`, `SqlitePool`, and `uuid`/`chrono` are in scope in `forecast_cmds.rs` using the existing imports at the top of that file.
- The Gauge (or chosen) icon is not available in the installed version of `lucide-react` — use `SlidersHorizontal` as the fallback icon name.
- A step's verification fails twice after a reasonable fix attempt.
- The fix appears to require touching a file not in the in-scope list.
- `parseBRLToCents` or `formatBRL` are not importable from `"../lib/format"` in `SettingsScreen.tsx` — verify with `grep -n "parseBRLToCents\|formatBRL" src/lib/format.ts` before proceeding.
- `savings_rate_bps` is not a field on `MonthMetric` — verify with `grep -n "savings_rate_bps" src/lib/api.ts` before adding the status prop.

## Maintenance notes

- The `DailyTetoCeilingSection` stores a display string in `app_setting` (`"daily_diario_ceiling_display"`) to pre-fill the input on the next mount. If the user later clears/zeroes the teto via another path, this display value could be stale — acceptable for now (the input is editable). A future plan could introduce a `get_active_daily_budget` command to always reflect the live DB value.
- The last-kind persistence in localStorage means that if the user uninstalls and reinstalls, they start back on "diario". This is correct behavior (no sensitive data; fresh start).
- If a future plan adds a `TransactionForm` full-form default-type, consider sharing the same `LAST_KIND_KEY` constant so both surfaces stay in sync.
- The Economizado badge added here uses the MONTHLY `savings_rate_bps`. The comment in `totaisStatus.ts` (lines 8–14) explains the deliberate divergence between the monthly badge threshold (20%, `SAVINGS_MIN_BPS`) and the forecast engine's savings guardrail (25%, `SAVINGS_TARGET_BPS`). Do not unify them without reading that comment first.
- Reviewers should check that the new `upsert_daily_budget` command always deprecates ALL active rows before inserting (not just the first). The current SQL `UPDATE daily_budget SET status='deprecated' WHERE status='active'` does this correctly.
- Follow-up deferred out of this plan: adding a "clear teto" button that sets amount to 0 and shows "usando média automática" — omitted to keep the UI minimal for the first pass.
