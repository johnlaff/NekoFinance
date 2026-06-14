# Spec 012 — Identidade estável de transação + reconciliação não-destrutiva (Conciliação P0)

> Fonte: análise local de conciliação (notas privadas). Slice 2 da meta. **Ordem inegociável.**

## Problema (risco de perda de dados — P0)

`import_rows` (`google_sheets/import.rs`) faz **`DELETE`-all por aba + `uuid::new_v4()` por
linha** a cada re-import. Consequências:

1. **Identidade aleatória**: toda re-importação regenera os ids → qualquer enriquecimento
   (split por pessoa, tags, payment_method, fatura) ancorado no `transaction.id` **morre**.
2. **Perda de fidelidade de tipo**: o parser só mapeia `amount_in`/`amount_out` e **não seta
   `is_fixed`** → toda Saída importada entra como `is_fixed=0` → o engine a classifica como
   **Diário**, não Saída (FixedOut). (Parte do WRONG #2.)
3. **Princípio fonte-da-verdade invertido** na spec 008 (declara "SQLite = system-of-record"),
   quando hoje a **planilha** é o system-of-record (o João a edita à mão todo dia).

## Decisões

### (b) Identidade determinística + UPSERT

- `ImportedRow` ganha `kind: RowKind` (`Entrada` | `Saida` | `Diario`), setado pelo parser
  conforme a coluna lida (`amount_in`→Entrada, `amount_out`→Saída; Diário quando mapeado).
- **Id determinístico** = `sha256("v1|" + sheet + "|" + date + "|" + kind + "|" + slot)`, onde
  `slot` = índice de ocorrência entre linhas com a mesma `(sheet,date,kind)` (≈ sempre 0; a
  planilha tem 1 célula por dia por coluna). O id NÃO inclui `amount`/`description` → **editar o
  valor ou a nota preserva o id** (UPSERT atualiza em vigor; o enriquecimento sobrevive).
- `import_rows`: para cada linha, `INSERT INTO "transaction"(...) ON CONFLICT(id) DO UPDATE SET
type, amount, description, date, is_fixed, is_projection, updated_at` — **só** as colunas que o
  import possui; preserva `payment_method`, `from/to_account_id`, e os `split`/tags que referenciam
  o id. `sync_log` idem (UPSERT por id).
- **Diff-delete**: apaga apenas as transações cujo id está no `sync_log` desta aba e **não** está
  no conjunto importado agora (linha removida da planilha). Substitui o `DELETE`-all.

### Tipo + is_fixed a partir do kind

- `Entrada` → `type=income`.
- `Saida` → `type=expense`, `is_fixed=1` (estilo de vida fixo + lump de fatura). → engine: FixedOut.
- `Diario` → `type=expense`, `is_fixed=0`. → engine: Daily.

### (a) Princípio fonte-da-verdade

- Corrigir `specs/008-auto-import/spec.md:30`: **a planilha é o system-of-record** enquanto a fase
  é "import-only"; o SQLite é o espelho local + camada de enriquecimento. Declarar a fonte por fase.

### (e) Par espelhado (empréstimo) → transfer _(sub-item; pode ser slice própria)_

- Detectar par Entrada=Saída no mesmo dia/nota (contador `X/36`) e emitir `type=transfer` em vez de
  inflar Entradas e Saídas. (Fica para 012b se a deteção exigir heurística de nota.)

## Fora de escopo desta slice (vão para 013+)

- (c) checksum **por célula** + gate de conflito EC11 (drift de edição manual concorrente).
- (d) preservação fiel de notas multi-item + split por pessoa no import.
- Write-back (parser inverso + diff + ApprovalDiffCard) — atrás de flag desligada.

## DoD

- `ImportedRow.kind` + parser setando kind; `compute_checksum` inclui kind.
- `row_id` determinístico (fn pura + teste de estabilidade).
- `import_rows` UPSERT + diff-delete + `is_fixed` por kind.
- Teste de regressão **`reimport_preserves_transaction_identity`**: importa, anexa um `split`,
  re-importa com valor editado → o `transaction.id` é o MESMO e o `split` sobrevive.
- Testes existentes do import continuam verdes (`reimport_*`, `replace_is_scoped_*`).
- `cargo test` + `npm run check` verdes.
