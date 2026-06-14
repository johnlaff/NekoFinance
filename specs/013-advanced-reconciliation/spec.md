# Spec 013 — Conciliação avançada: drift por célula + gate de conflito + preservar enriquecimento

> Fonte: análise local de conciliação (notas privadas). Continuação da spec 012 (identidade
> estável). Encerra os itens (c) e (d) da conciliação P0.

## Problema

A spec 012 deu identidade determinística + UPSERT + diff-delete: o enriquecimento ancorado no
`transaction.id` (split, tags, payment_method, contas) sobrevive a um re-import. Mas restam dois
buracos:

1. **(c) Sem detecção de drift / conflito.** O re-import faz a planilha **vencer sempre**: o
   UPSERT sobrescreve `amount`/`description` com o valor da célula. Se o usuário **editou o valor
   localmente** E a **célula também mudou** (de forma diferente) na planilha, a edição local some
   **silenciosamente**. Não há gate para o humano decidir.
2. **(d) A descrição/nota local é clobberada.** O `description` vem do note da célula e o UPSERT
   faz `description=excluded.description` em todo import → uma descrição curada localmente
   (ex.: split de um lump em itens) é sobrescrita pela nota crua da planilha.

## Decisão — merge de 3 vias (base / local / planilha)

A chave é guardar o **base**: o valor do campo **como foi importado da planilha da última vez**.
Com base + local (valor atual no app) + sheet (valor agora na planilha), cada campo decide:

| base | local vs base | sheet vs base | Decisão |
|---|---|---|---|
| ausente (1º import) | — | — | **AplicarPlanilha** (semeia base) |
| presente | local == base | sheet == base | **ManterLocal** (nada mudou) |
| presente | local == base | sheet ≠ base | **AplicarPlanilha** (só a planilha mudou) |
| presente | local ≠ base | sheet == base | **ManterLocal** (só o local mudou → preserva edição) |
| presente | local ≠ base | sheet ≠ base | **Conflito** (ambos mudaram → gate humano) |

### Núcleo puro

`reconcile<T: Eq>(base: Option<&T>, local: &T, sheet: &T) -> MergeDecision` em
`google_sheets/reconcile.rs` (functional core, sem IO). `MergeDecision = { ApplySheet, KeepLocal,
Conflict }`. Aplica-se por campo (amount, description). Totalmente testado.

### Armazenamento do base + conflitos

- `transaction` ganha `source_amount INTEGER` e `source_description TEXT` (o snapshot do último
  import). NULL = nunca importado da planilha (lançamento 100% manual; nunca entra em conflito de
  import).
- Tabela `import_conflict (id, transaction_id, field, base_value, local_value, sheet_value,
  created_at, resolved_at, resolution)` — fila do gate de conflito (persiste para a UI).

### Import (shell)

Para cada linha importada, antes de gravar:
1. Carrega o txn atual (se existir) e seu `source_*`.
2. Para `amount` e `description`, roda `reconcile(source, local, sheet)`:
   - **ApplySheet** → grava o valor da planilha + atualiza `source_*`.
   - **KeepLocal** → mantém o valor local; atualiza `source_*` só quando a planilha não mudou
     (mantém o base alinhado ao que está na planilha quando ela não mudou).
   - **Conflito** → **não** grava o campo; insere/atualiza `import_conflict` (idempotente por
     `transaction_id+field`). O resto da linha (campos sem conflito) grava normalmente.
3. `is_fixed`, `is_projection`, `kind` seguem a planilha (estruturais, não editados localmente).

### Resolução (gate)

- `get_import_conflicts()` → lista os conflitos não resolvidos (com base/local/sheet) para a UI
  renderizar via `ApprovalDiffCard`.
- `resolve_import_conflict(id, "sheet" | "local")` → aplica a escolha ao `transaction`, alinha
  `source_*` ao valor escolhido, marca `resolved_at`/`resolution`.

## Não-objetivos

- Write-back ao Sheets (spec 018, atrás de flag).
- Conciliação com Open Finance/banco (futuro).

## Testes (TDD obrigatório — sync)

- `reconcile`: as 6 linhas da tabela acima.
- Import: edição local preservada quando a planilha não muda; update da planilha aplicado quando
  só ela muda; conflito registrado quando ambos mudam; lançamento manual (source NULL) nunca
  conflita; campos sem conflito gravam mesmo quando outro campo conflita.
- Resolução: `sheet` e `local` aplicam o valor certo e limpam o conflito.
