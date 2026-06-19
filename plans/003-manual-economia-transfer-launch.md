# Plan 003: Allow manual Economia/transfer launch (engine + form)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat d183bbf..HEAD -- src-tauri/src/commands.rs src/lib/api.ts src/screens/NewTransactionForm.tsx src/screens/NewTransactionForm.test.tsx`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `d183bbf`, 2026-06-19

## Why this matters

The method defines Economia as a monthly transfer of savings to a reserve or
illiquid account — the numerator of the "Economizado%" metric (target 20–30%
annually). Today, the only write path for Economia is spreadsheet import: the
Rust command `create_transaction_inner` (`commands.rs:1805`) explicitly rejects
any `txn_type` that is not `"income"` or `"expense"`, so a transfer to the
reserve can never be created manually. The forecast engine already buckets
`type='transfer'` to a `reserve`/`illiquid` account as Economia; the UI badge
`MovBadge` already renders the "economia" kind; only the write path and the
form option are missing. Without this, users who want to record a manual
Economia (common between spreadsheet imports) have no in-app path and see a
misleadingly zero Economizado%.

## Current state

### Files and roles

- `src-tauri/src/commands.rs` — All Tauri commands. Contains
  `create_transaction` (public command, lines 1768–1791) and its inner
  function `create_transaction_inner` (lines 1794–1862) which holds the
  rejection guard. Also contains `ensure_reserve_account` (lines 2314–2340),
  `pockets`/`get_pockets` (lines 1534–1558), and all Rust unit tests.
- `src/lib/api.ts` — TypeScript bindings for all Tauri commands. Contains
  `createTransaction` (lines 261–272) and `getPockets` (line 234–236),
  `PocketAccount` interface (lines 215–222).
- `src/screens/NewTransactionForm.tsx` — The manual entry form. Contains
  `FORM_KINDS` (line 9), `kindToFields` (lines 12–28), the reducer
  `FormState`/`formReducer` (lines 87–148), and the JSX form (lines 208–431).
- `src/screens/NewTransactionForm.test.tsx` — Vitest tests for the form using
  `mockCommands`/`mockInvoke` from `src/test/commands.ts`.

### Key excerpts (live code at planned-at commit)

**The rejection guard** (`src-tauri/src/commands.rs:1805–1806`):
```rust
if !matches!(txn_type, "income" | "expense") {
    return Err(format!("tipo inválido: {txn_type}"));
}
```

**The transfer INSERT used by the Economia importer** (`commands.rs:2373–2374`):
```sql
INSERT INTO "transaction" (id, type, amount, description, date,
  to_account_id, is_projection, created_at, updated_at)
VALUES (?1, 'transfer', ?2, 'Economia (importada da aba Economia)',
  ?3, ?4, ?5, ?6, ?6)
```
The schema allows `from_account_id` and `to_account_id` on the transaction
row (`migrations/20240608000006_transaction.sql:9–10`).

**The engine's Economia classification** (`commands.rs:536–538`):
```sql
LEFT JOIN account a ON a.id = t.to_account_id
WHERE ... AND t.type='transfer'
  AND a.liquidity IN ('reserve','illiquid')
```
This already works correctly for imported transfers. A manually created
transfer to a reserve/illiquid account will be classified the same way.

**The public Tauri command signature** (`commands.rs:1768–1791`):
```rust
pub async fn create_transaction(
    pool: State<'_, SqlitePool>,
    txn_type: String,
    amount_cents: i64,
    description: Option<String>,
    date: String,
    payment_method: Option<String>,
    is_fixed: bool,
    tag_ids: Vec<String>,
    recurrence: Option<RecurrenceInput>,
) -> Result<String, String>
```
Adding `to_account_id: Option<String>` to this signature requires a matching
update to the TypeScript binding.

**The TypeScript binding** (`src/lib/api.ts:261–272`):
```ts
export function createTransaction(input: {
  txnType: "income" | "expense";
  amountCents: number;
  description: string | null;
  date: string;
  paymentMethod: string | null;
  isFixed: boolean;
  tagIds: string[];
  recurrence: { frequency: Frequency; repetitions: number } | null;
}): Promise<string> {
  return invoke("create_transaction", input);
}
```

**The form's movement-type list** (`src/screens/NewTransactionForm.tsx:8–9`):
```ts
/** Os tipos de movimento oferecidos no form (Economia → transfer precisa de conta, fica fora). */
const FORM_KINDS: MovKind[] = ["entrada", "saida", "diario", "cartao"];
```
The comment already anticipates that Economia needs a destination account.
`MovKind` is defined in `src/design-system/components/MovBadge.tsx:8` as:
```ts
export type MovKind = "entrada" | "saida" | "diario" | "economia" | "cartao";
```

**The movement-to-fields mapper** (`NewTransactionForm.tsx:12–28`):
```ts
function kindToFields(kind: MovKind): {
  txnType: "income" | "expense";
  isFixed: boolean;
  paymentMethod: string | null;
} {
  switch (kind) {
    case "entrada":
      return { txnType: "income", isFixed: false, paymentMethod: null };
    case "saida":
      return { txnType: "expense", isFixed: true, paymentMethod: "debit" };
    case "cartao":
      return { txnType: "expense", isFixed: false, paymentMethod: "credit" };
    case "diario":
    default:
      return { txnType: "expense", isFixed: false, paymentMethod: "debit" };
  }
}
```

**The existing Economia placeholder UI** (`NewTransactionForm.tsx:253–256`):
```tsx
<MovBadge kind="economia" size={14} /> Economia entra pela aba Economia da
planilha (Configurações › Conexão Google Sheets) — é uma transferência
para a sua reserva, não um gasto.
```

**The existing bad-input Rust test** (`commands.rs:3319–3352`):
```rust
async fn create_transaction_rejects_bad_input() {
    // ...
    create_transaction_inner(&pool, "transfer", 100, None,
        "2026-06-14", None, false, &[], None)
        .await.is_err(), "tipo não suportado pelo form é rejeitado"
```
This test asserts that `"transfer"` is rejected. It must be UPDATED (not
deleted) to test that `"transfer"` with a valid reserve account ID succeeds,
and that `"transfer"` with a missing or liquid `to_account_id` fails.

**The `ensure_reserve_account` helper** (`commands.rs:2314–2340`): this
internal helper already knows how to find or create a reserve account — the
new path can use it as a model for how reserve validation works, but does NOT
call it automatically. The form must require the user to pick an explicit
reserve account.

**The `insert_reserve_account` test fixture** (`commands.rs:3513–3531`):
```rust
async fn insert_reserve_account(pool: &sqlx::SqlitePool, balance: i64) {
    // inserts a person + a savings/reserve account
}
```
Use this helper in new Rust tests for the transfer path.

### Repo conventions that apply here

- **Functional-core / imperative-shell**: validation logic goes in
  `create_transaction_inner` (pure, testable). The `#[tauri::command]` wrapper
  only delegates.
- **Amount is positive magnitude**: the existing guard `amount_cents <= 0`
  stays; transfers must also be positive magnitude.
- **React Compiler is enabled**: do NOT add `useMemo`, `useCallback`, or
  manual `memo()` to the form. Use `useReducer` for new form state (the form
  already uses one — extend it).
- **Design System tokens only**: use `var(--...)` tokens from
  `src/design-system/`. No inline pixel values for spacing/font. The pattern
  to follow is the existing `field` and `label` style constants at the top of
  `NewTransactionForm.tsx`.
- **Test pattern for the form**: see `NewTransactionForm.test.tsx` — each case
  uses `mockCommands({ list_tags_cmd: ..., create_transaction: ... })`,
  renders the form, interacts via `userEvent`, and asserts on
  `mockInvoke.mock.calls`.

### CONTEXT.md vocabulary to use in names and comments

- `Transaction.type` values: `'income'`, `'expense'`, `'transfer'`
- `Transaction.from_account_id` / `Transaction.to_account_id` (schema columns)
- `Account.liquidity` values: `'liquid'`, `'reserve'`, `'illiquid'`, `'restricted'`
- Economia = `type='transfer'` to a `reserve`/`illiquid` account
- `Economizado%` = `registered_economia_cents / realized_income_cents`
- Do NOT use the words "caixinha", "sub-goal", or "pocket goal" — the method
  rejects sub-goals; Economia goes to the single reserve.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust check (fmt+clippy+test) | `npm run rust:check` | exit 0 |
| Rust tests only | `cargo test --manifest-path src-tauri/Cargo.toml --locked` | all pass |
| Typecheck | `npm run typecheck` | exit 0, no errors |
| Lint | `npm run lint` | exit 0 |
| Front unit tests | `npm run test:run` | all pass |
| Full gate | `npm run check` | exit 0 |

## Suggested executor toolkit

- Read `CONTEXT.md` at repo root before starting — it defines the authoritative
  vocabulary for Transaction types and Account liquidity classes.
- The `MovBadge` component (`src/design-system/components/MovBadge.tsx`) is the
  reference for the 5 method movement types.

## Scope

**In scope** (the only files you should modify):

- `src-tauri/src/commands.rs` — `create_transaction` command signature,
  `create_transaction_inner` validation and INSERT, Rust unit tests.
- `src/lib/api.ts` — `createTransaction` function signature (add
  `toAccountId`).
- `src/screens/NewTransactionForm.tsx` — add `"economia"` to `FORM_KINDS`,
  extend `FormState` with `toAccountId`, extend `kindToFields`, add
  destination-account picker UI shown only when kind is `"economia"`.
- `src/screens/NewTransactionForm.test.tsx` — add tests for the Economia path.

**Out of scope** (do NOT touch, even though they look related):

- `src-tauri/src/google_sheets/import.rs` and `write_back.rs` — the Economia
  importer path is separate and already works; this plan is manual-only.
- Multi-titular split UI (`Split.owner_person_id`) — deferred to plan 015.
- `src/features/` and other screens — only the form and its test are in scope.
- `src-tauri/src/recurrence.rs` — Economia transfers are one-off lançamentos;
  recurrence for transfers is out of scope.
- Any change to the `recurrence` code path in `create_transaction_inner` — the
  recurrence branch (lines 1814–1833) validates only income/expense types; do
  not add `transfer` to recurrence without a separate plan.

## Git workflow

- Branch: `advisor/003-economia-transfer-launch`
- Commit message style: conventional commits with a body when needed, matching
  the repo's recent log. Examples from `git log`:
  `fix: allow manual Economia transfer in create_transaction`
  `feat: add Economia option to NewTransactionForm with reserve picker`
- Do NOT push or open a PR unless the operator instructs it.

## Steps

### Step 1: Extend `create_transaction_inner` to accept `"transfer"` with a validated reserve/illiquid destination

Open `src-tauri/src/commands.rs`.

**Changes to `create_transaction` (public command, lines 1768–1791)**:
Add `to_account_id: Option<String>` as a new parameter after `tag_ids`:

```rust
pub async fn create_transaction(
    pool: State<'_, SqlitePool>,
    txn_type: String,
    amount_cents: i64,
    description: Option<String>,
    date: String,
    payment_method: Option<String>,
    is_fixed: bool,
    tag_ids: Vec<String>,
    recurrence: Option<RecurrenceInput>,
    to_account_id: Option<String>,   // NEW: required when txn_type = "transfer"
) -> Result<String, String> {
    create_transaction_inner(
        pool.inner(),
        &txn_type,
        amount_cents,
        description,
        &date,
        payment_method,
        is_fixed,
        &tag_ids,
        recurrence,
        to_account_id.as_deref(),    // NEW
    )
    .await
}
```

**Changes to `create_transaction_inner` (lines 1794–1862)**:

Add `to_account_id: Option<&str>` as the last parameter.

Replace the rejection guard at lines 1805–1807 with logic that:
1. Still rejects types other than `"income"`, `"expense"`, `"transfer"`.
2. For `"transfer"`, validates that `to_account_id` is `Some(non-empty)` and
   that the referenced account's `liquidity` is `"reserve"` or `"illiquid"`.
3. For `"income"` and `"expense"`, `to_account_id` must be `None` (ignore
   silently or reject — rejecting makes bugs louder; prefer rejection).

The new guard (replace the old two-liner at lines 1805–1807 plus add a
validation block before the `let start = ...` line):

```rust
match txn_type {
    "income" | "expense" => {}
    "transfer" => {
        // Economia: transfer must target a reserve or illiquid account.
        let dest_id = to_account_id
            .filter(|s| !s.is_empty())
            .ok_or("transfer requer conta-destino (to_account_id)")?;
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT COALESCE(liquidity,'') FROM account WHERE id = ?1",
        )
        .bind(dest_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("query account: {e}"))?;
        match row {
            None => return Err("conta-destino não encontrada".into()),
            Some((liq,)) if liq == "reserve" || liq == "illiquid" => {}
            Some((liq,)) => {
                return Err(format!(
                    "conta-destino deve ter liquidez 'reserve' ou 'illiquid', encontrado '{liq}'"
                ))
            }
        }
    }
    other => return Err(format!("tipo inválido: {other}")),
}
```

The recurrence path (lines 1814–1833) must NOT accept `"transfer"` — add a
guard at the top of the recurrence `if let Some(rec) = recurrence {` block:

```rust
if let Some(rec) = recurrence {
    if txn_type == "transfer" {
        return Err("Economia não suporta recorrência".into());
    }
    // ... rest of existing recurrence logic unchanged
```

The single-transaction INSERT (lines 1841–1856) does not include
`to_account_id`. Extend it to bind `to_account_id` when the type is
`"transfer"`. Replace the query and binds:

```rust
sqlx::query(
    "INSERT INTO \"transaction\" \
     (id, type, amount, description, date, payment_method, \
      is_fixed, to_account_id, is_projection, created_at, updated_at) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
)
.bind(&id)
.bind(txn_type)
.bind(amount_cents)
.bind(&description)
.bind(date)
.bind(&payment_method)
.bind(is_fixed as i64)
.bind(if txn_type == "transfer" { to_account_id } else { None })  // NEW
.bind(is_projection as i64)
.bind(&now)
.execute(pool)
.await
.map_err(|e| format!("insert transaction: {e}"))?;
```

Note: `from_account_id` is intentionally left NULL (the method does not
require specifying the source liquid account for Economia; the existing
importer also leaves it NULL — see `commands.rs:2373`).

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked 2>&1 | tail -20`
Expected: compile errors from the changed signature (the existing
`create_transaction_rejects_bad_input` test calls `create_transaction_inner`
without the new parameter — that is intentional; fix it in step 2 before
expecting green tests).

### Step 2: Update the existing Rust unit tests to match the new signature

Open `src-tauri/src/commands.rs`. Find all calls to `create_transaction_inner`
in the test section (lines ~3234–3352) and add `None` as the final argument
(`to_account_id`) to every existing call site. There are four call sites:

1. `create_transaction_inserts_realized_with_tags` (~line 3240)
2. `create_transaction_with_recurrence_builds_tagged_series` (~line 3282)
3. `create_transaction_rejects_bad_input` (~line 3322) — two calls here

The existing test `create_transaction_rejects_bad_input` currently asserts that
`"transfer"` is rejected (`is_err()`). Update it: the assertion that
`"transfer"` without a `to_account_id` is `Err` should remain (passes `None`),
and add a new assertion that `"transfer"` with a LIQUID destination is also
`Err`. Do NOT yet add the happy-path transfer test here; that comes in step 3.

After the fix, the test becomes:

```rust
async fn create_transaction_rejects_bad_input() {
    let pool = fixture_pool().await;

    // "transfer" with no destination → Err (the new guard rejects it)
    assert!(
        create_transaction_inner(
            &pool, "transfer", 100, None, "2026-06-14",
            None, false, &[], None, None,
        )
        .await
        .is_err(),
        "transfer sem to_account_id é rejeitado"
    );

    // invalid type → Err (unchanged)
    assert!(
        create_transaction_inner(
            &pool, "bogus", 100, None, "2026-06-14",
            None, false, &[], None, None,
        )
        .await
        .is_err(),
        "tipo inválido é rejeitado"
    );

    // zero amount → Err (unchanged)
    assert!(
        create_transaction_inner(
            &pool, "expense", 0, None, "2026-06-14",
            None, false, &[], None, None,
        )
        .await
        .is_err(),
        "valor zero/negativo é rejeitado"
    );
}
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked 2>&1 | tail -20`
Expected: all tests pass (no compile errors, no test failures).

### Step 3: Add Rust unit tests for the new transfer happy path and validation

Still in `src-tauri/src/commands.rs`, inside the `#[cfg(test)]` block, add
two new `#[tokio::test]` functions after `create_transaction_rejects_bad_input`:

**Test A — happy path: transfer to reserve account is accepted and written correctly**:

```rust
#[tokio::test]
async fn create_transaction_transfer_to_reserve_inserts_economia() {
    let pool = fixture_pool().await;
    // Need a reserve account (savings/reserve) and a person.
    insert_reserve_account(&pool, 0).await;
    let (reserve_id,): (String,) =
        sqlx::query_as("SELECT id FROM account WHERE liquidity='reserve' LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();

    let id = create_transaction_inner(
        &pool, "transfer", 50_000,
        Some("Economia manual".into()),
        "2026-06-19", None, false, &[], None,
        Some(&reserve_id),
    )
    .await
    .expect("transfer para reserva deve ser aceito");

    let (r#type, amount, to_acct): (String, i64, Option<String>) = sqlx::query_as(
        "SELECT type, amount, to_account_id FROM \"transaction\" WHERE id = ?1",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(r#type, "transfer");
    assert_eq!(amount, 50_000);
    assert_eq!(to_acct.as_deref(), Some(reserve_id.as_str()));
}
```

**Test B — liquid destination is rejected**:

```rust
#[tokio::test]
async fn create_transaction_transfer_to_liquid_account_is_rejected() {
    let pool = fixture_pool().await;
    insert_liquid_account(&pool, 100_000).await;
    let (liquid_id,): (String,) =
        sqlx::query_as("SELECT id FROM account WHERE liquidity='liquid' LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();

    let result = create_transaction_inner(
        &pool, "transfer", 10_000, None,
        "2026-06-19", None, false, &[], None,
        Some(&liquid_id),
    )
    .await;
    assert!(
        result.is_err(),
        "transfer para conta líquida não é Economia — deve ser rejeitado"
    );
}
```

**Verify**: `cargo test --manifest-path src-tauri/Cargo.toml --locked 2>&1 | grep -E "test result|FAILED|create_transaction"`
Expected: all tests pass; `create_transaction_transfer_to_reserve_inserts_economia` and
`create_transaction_transfer_to_liquid_account_is_rejected` appear in the passing list.

### Step 4: Run the full Rust check gate

**Verify**: `npm run rust:check`
Expected: exit 0. All of fmt, clippy, and tests pass.

### Step 5: Extend the TypeScript `createTransaction` binding

Open `src/lib/api.ts`. Extend the `createTransaction` input type and the
`getPockets` return type is already correct (no changes needed there). Only
`createTransaction` needs updating:

Change the union type `"income" | "expense"` to `"income" | "expense" | "transfer"`
and add `toAccountId: string | null`:

```ts
export function createTransaction(input: {
  txnType: "income" | "expense" | "transfer";
  amountCents: number;
  description: string | null;
  date: string;
  paymentMethod: string | null;
  isFixed: boolean;
  tagIds: string[];
  recurrence: { frequency: Frequency; repetitions: number } | null;
  toAccountId: string | null;   // NEW: required (non-null) when txnType = "transfer"
}): Promise<string> {
  return invoke("create_transaction", input);
}
```

**Verify**: `npm run typecheck`
Expected: TypeScript errors in `NewTransactionForm.tsx` because the form now
passes an input object missing the new `toAccountId` field. This is expected
and will be fixed in step 6.

### Step 6: Extend `NewTransactionForm` to support the Economia kind

Open `src/screens/NewTransactionForm.tsx`. Make the following changes in order:

**6a. Add `toAccountId` to `FormState`** (around line 87):
```ts
interface FormState {
  kind: MovKind;
  amount: string;
  description: string;
  date: string;
  selectedTags: string[];
  toAccountId: string;   // NEW: id of destination account for Economia
  repeat: boolean;
  frequency: Frequency;
  repetitions: number;
  busy: boolean;
  error: string | null;
}
```

**6b. Initialize `toAccountId` in `makeInitialForm`** (around line 100):
```ts
function makeInitialForm(): FormState {
  return {
    kind: "diario",
    amount: "",
    description: "",
    date: todayISO(),
    selectedTags: [],
    toAccountId: "",   // NEW
    repeat: false,
    frequency: "mensal",
    repetitions: 12,
    busy: false,
    error: null,
  };
}
```

**6c. Add `"economia"` to `FORM_KINDS`** (line 9):
```ts
const FORM_KINDS: MovKind[] = ["entrada", "saida", "diario", "cartao", "economia"];
```

**6d. Extend `kindToFields`** to return `txnType: "income" | "expense" | "transfer"`.
Update the return type and add the `"economia"` case:
```ts
function kindToFields(kind: MovKind): {
  txnType: "income" | "expense" | "transfer";
  isFixed: boolean;
  paymentMethod: string | null;
} {
  switch (kind) {
    case "entrada":
      return { txnType: "income", isFixed: false, paymentMethod: null };
    case "saida":
      return { txnType: "expense", isFixed: true, paymentMethod: "debit" };
    case "cartao":
      return { txnType: "expense", isFixed: false, paymentMethod: "credit" };
    case "economia":
      return { txnType: "transfer", isFixed: false, paymentMethod: null };
    case "diario":
    default:
      return { txnType: "expense", isFixed: false, paymentMethod: "debit" };
  }
}
```

**6e. Load available reserve/illiquid accounts**: add a `useState` for
accounts and a `useEffect` that calls `getPockets()` and filters accounts
where `liquidity === "reserve" || liquidity === "illiquid"`. Place it near the
existing `useEffect` for tags (around line 166). Import `getPockets` and
`PocketAccount` from `../lib/api`.

```ts
import { createTransaction, getPockets, listTags, type Frequency, type PocketAccount, type Tag } from "../lib/api";

// Inside NewTransactionForm component, after the tags useState:
const [reserveAccounts, setReserveAccounts] = useState<PocketAccount[]>([]);

useEffect(() => {
  let alive = true;
  getPockets()
    .then((p) => {
      if (!alive) return;
      setReserveAccounts(
        p.accounts.filter(
          (a) => a.liquidity === "reserve" || a.liquidity === "illiquid"
        )
      );
    })
    .catch(() => alive && setReserveAccounts([]));
  return () => { alive = false; };
}, []);
```

**6f. Destructure `toAccountId` from form state** alongside the other
destructured values (after line 163).

**6g. Extend `canSubmit`** to require a non-empty `toAccountId` when
`kind === "economia"`:
```ts
const canSubmit =
  amountCents != null &&
  amountCents > 0 &&
  !busy &&
  (kind !== "economia" || toAccountId !== "");
```

**6h. Extend the `submit` function** to pass `toAccountId` to `createTransaction`:
```ts
await createTransaction({
  txnType: fields.txnType,
  amountCents,
  description: description.trim() || null,
  date,
  paymentMethod: fields.paymentMethod,
  isFixed: fields.isFixed,
  tagIds: selectedTags,
  recurrence: repeat ? { frequency, repetitions } : null,
  toAccountId: kind === "economia" ? toAccountId : null,   // NEW
});
```

**6i. Replace the "Economia entra pela aba Economia" paragraph** (lines
253–256) with a destination-account picker rendered only when
`kind === "economia"`. If there are no reserve/illiquid accounts available,
show a helpful message directing the user to create one first (in
Configurações > Bolsos). Keep the `<MovBadge kind="economia" />` hint in the
help text only when `kind !== "economia"` (move it inside the other kinds'
informational paragraph, or remove it entirely from the static hint since the
kind chip itself already shows the badge).

Replace the entire static paragraph block with:

```tsx
{kind === "economia" ? (
  <div>
    <label htmlFor="ntf-dest-account" style={label}>
      Conta-destino (reserva)
    </label>
    {reserveAccounts.length === 0 ? (
      <p
        style={{
          margin: 0,
          fontSize: "var(--fs-sm)",
          color: "var(--text-faint)",
        }}
      >
        Nenhuma conta de reserva ou ilíquida encontrada. Crie uma em{" "}
        Configurações &rsaquo; Bolsos antes de registrar Economia.
      </p>
    ) : (
      <select
        id="ntf-dest-account"
        value={toAccountId}
        onChange={(e) =>
          dispatch({ type: "set", patch: { toAccountId: e.target.value } })
        }
        style={{ ...field, width: "100%" }}
      >
        <option value="">Selecione a conta…</option>
        {reserveAccounts.map((a) => (
          <option key={a.id} value={a.id}>
            {a.name}
            {a.liquidity === "illiquid" ? " (ilíquida)" : ""}
          </option>
        ))}
      </select>
    )}
  </div>
) : (
  <p
    style={{
      margin: "var(--space-2) 0 0",
      fontSize: "var(--fs-micro)",
      color: "var(--text-faint)",
    }}
  >
    <MovBadge kind="economia" size={14} /> Economia é uma transferência
    para a sua reserva — registre aqui ou importe pela aba Economia da
    planilha (Configurações &rsaquo; Conexão Google Sheets).
  </p>
)}
```

**6j. Hide the "Repetir" toggle when `kind === "economia"`** to prevent the
user from accidentally creating a recurrence the backend rejects. Wrap the
repeat block with `{kind !== "economia" && (...)`.

**Verify**: `npm run typecheck` → exit 0, no errors.

### Step 7: Add front-end tests for the Economia path

Open `src/screens/NewTransactionForm.test.tsx`. Add two new test cases:

**Test A — Economia happy path: picks reserve account and calls create_transaction as transfer**:

```ts
it("lança Economia como transfer para conta reserva", async () => {
  const user = userEvent.setup();
  mockCommands({
    list_tags_cmd: [],
    get_pockets: {
      liquid_cents: 0,
      reserve_cents: 1500000,
      restricted_cents: 0,
      illiquid_cents: 0,
      net_worth_cents: 1500000,
      accounts: [
        {
          id: "reserve-001",
          name: "Poupança",
          type: "savings",
          liquidity: "reserve",
          balance: 1500000,
          institution: null,
        },
      ],
    },
    create_transaction: "tx-economia-id",
  });
  render(<NewTransactionForm />);

  // Wait for the Economia button to appear (FORM_KINDS now includes it).
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: /Economia/ })
    ).toBeInTheDocument()
  );

  await user.click(screen.getByRole("button", { name: /Economia/ }));

  // Wait for reserve accounts to load.
  await waitFor(() =>
    expect(screen.getByLabelText("Conta-destino (reserva)")).toBeInTheDocument()
  );

  await user.type(screen.getByLabelText("Valor"), "1.000,00");
  await user.selectOptions(
    screen.getByLabelText("Conta-destino (reserva)"),
    "reserve-001"
  );
  await user.click(screen.getByRole("button", { name: "Lançar" }));

  await waitFor(() => {
    const call = mockInvoke.mock.calls.find((c) => c[0] === "create_transaction");
    expect(call?.[1]).toMatchObject({
      txnType: "transfer",
      amountCents: 100000,
      paymentMethod: null,
      isFixed: false,
      toAccountId: "reserve-001",
      recurrence: null,
    });
  });
});
```

**Test B — Economia without reserve accounts: submit button disabled**:

```ts
it("desabilita Lançar sem conta reserva disponível", async () => {
  const user = userEvent.setup();
  mockCommands({
    list_tags_cmd: [],
    get_pockets: {
      liquid_cents: 842000,
      reserve_cents: 0,
      restricted_cents: 0,
      illiquid_cents: 0,
      net_worth_cents: 842000,
      accounts: [
        {
          id: "bank-001",
          name: "Conta corrente",
          type: "bank",
          liquidity: "liquid",
          balance: 842000,
          institution: null,
        },
      ],
    },
    create_transaction: "never-called",
  });
  render(<NewTransactionForm />);

  await user.click(screen.getByRole("button", { name: /Economia/ }));
  await user.type(screen.getByLabelText("Valor"), "500,00");

  // No reserve account available → toAccountId stays empty → button disabled.
  expect(screen.getByRole("button", { name: "Lançar" })).toBeDisabled();
});
```

Import `POCKETS` from `../test/commands` is not used here; mock inline as shown
above because the POCKETS fixture contains a reserve account and would make
test B fail. The `mockCommands` `get_pockets` key must match the exact Tauri
command name used in `getPockets()` — which is `"get_pockets"` (check
`src/lib/api.ts:234`: `return invoke("get_pockets")`).

**Verify**: `npm run test:run 2>&1 | tail -30`
Expected: all tests pass, including the 2 new Economia tests (total front
test count increases by 2).

### Step 8: Run the full quality gate

**Verify**: `npm run check`
Expected: exit 0. All of typecheck, lint, front tests, Rust check pass.

## Test plan

### New Rust tests (in `src-tauri/src/commands.rs`)

| Test name | File | Case covered |
|---|---|---|
| `create_transaction_transfer_to_reserve_inserts_economia` | `commands.rs` | Happy path: `"transfer"` + reserve `to_account_id` → row inserted with correct type, amount, and `to_account_id` |
| `create_transaction_transfer_to_liquid_account_is_rejected` | `commands.rs` | Validation: `"transfer"` to a liquid account → `Err` |
| Updated `create_transaction_rejects_bad_input` | `commands.rs` | Regression: `"transfer"` with no `to_account_id` still `Err`; zero amount still `Err`; bogus type still `Err` |

Model the new Rust tests after `create_transaction_inserts_realized_with_tags`
(~line 3234) and use the existing `insert_reserve_account` and
`insert_liquid_account` test helpers.

### New front-end tests (in `src/screens/NewTransactionForm.test.tsx`)

| Test | File | Case |
|---|---|---|
| "lança Economia como transfer para conta reserva" | `NewTransactionForm.test.tsx` | Happy path: form picks reserve account, submits as `txnType: "transfer"` |
| "desabilita Lançar sem conta reserva disponível" | `NewTransactionForm.test.tsx` | No-reserve guard: submit disabled when no reserve accounts found |

Model the new form tests after the existing "lança um diário variável" test
(line 26), using `mockCommands` with a `get_pockets` handler.

**Verify**: `npm run test:run` → all pass, including 2 new front tests.
`cargo test --manifest-path src-tauri/Cargo.toml --locked` → all pass,
including 2 new Rust tests (1 updated).

## Done criteria

Machine-checkable. ALL must hold before marking this plan DONE:

- [ ] `npm run rust:check` exits 0
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked 2>&1 | grep "create_transaction_transfer_to_reserve_inserts_economia"` shows the test passing
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --locked 2>&1 | grep "create_transaction_transfer_to_liquid_account_is_rejected"` shows the test passing
- [ ] `npm run typecheck` exits 0
- [ ] `npm run lint` exits 0
- [ ] `npm run test:run` exits 0; output includes "lança Economia como transfer" and "desabilita Lançar sem conta reserva"
- [ ] `grep -n "\"income\" | \"expense\"" src-tauri/src/commands.rs` returns no matches (the old two-literal guard is gone)
- [ ] `grep -n "toAccountId" src/lib/api.ts` returns at least one match
- [ ] `grep -n "economia" src/screens/NewTransactionForm.tsx | grep FORM_KINDS` returns a match
- [ ] `git diff --name-only` lists only files in the in-scope list
- [ ] `plans/README.md` status row for plan 003 updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The code at lines 1805–1806 of `commands.rs` does not contain
  `!matches!(txn_type, "income" | "expense")` — the codebase has drifted.
- The code at line 9 of `NewTransactionForm.tsx` does not contain
  `["entrada", "saida", "diario", "cartao"]` — the codebase has drifted.
- The `create_transaction` Tauri command signature (lines 1768–1791) has
  already gained a `to_account_id` parameter — someone else partially
  implemented this and the plan may be stale.
- Step 3's Rust test (`create_transaction_transfer_to_reserve_inserts_economia`)
  fails after two fix attempts — the reserve validation logic may conflict with
  migration state or the `ensure_reserve_account` helper.
- `npm run typecheck` still shows errors after step 6 and two fix attempts —
  the `getPockets`/`PocketAccount` import may have a mismatch with the live
  `src/lib/api.ts` types.
- Any change is required to a file outside the in-scope list to make the
  tests pass — this is a scope violation; report it before proceeding.
- The `get_pockets` Tauri command name used in `src/lib/api.ts:234` differs
  from `"get_pockets"` — the mock in the new form tests uses that name and
  will silently fail if it is wrong.
- The method's behavior requires recording a `from_account_id` on Economia
  transfers — the plan intentionally leaves it NULL (matching the importer
  at `commands.rs:2373`); if you discover that the forecast engine or any
  query requires a non-NULL `from_account_id` on transfers, stop and report.

## Maintenance notes

- **Recurrence for Economia is explicitly blocked** in this plan. If the user
  later wants monthly auto-Economia, a separate plan should add `"transfer"` to
  the recurrence path in `create_transaction_inner` (lines 1814–1833) and in
  `src-tauri/src/recurrence.rs`.
- **`from_account_id` is NULL** on manually created Economia transfers (same as
  the importer). If a future multi-account net-worth query needs a non-NULL
  source, it must treat NULL as "from liquid accounts in aggregate" rather than
  failing.
- **PR reviewer should check**: the destination-account picker disappears when
  the user switches away from "economia" kind — ensure `toAccountId` is
  cleared (or ignored) when `kind !== "economia"` so a stale value does not
  pollute other lançamento types. A safe implementation sets `toAccountId: ""`
  in the `"set"` action whenever `kind` changes.
- **The "Repetir" section is hidden for Economia** (step 6j). If you see a
  regression where the repeat checkbox appears for economy lançamentos, it is
  a UI-only issue but will cause a backend error on submit.
- **Reserve account prerequisite**: if the user has no reserve or illiquid
  account, the form shows a help text directing them to Configurações > Bolsos.
  This is intentional — do not auto-create an account on form submit (that
  would be a silent schema mutation without user approval).
