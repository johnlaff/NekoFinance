# Spec 014 — Tags (N:N, cor, emoji, "! Pagar") + categorias→tags

> Fonte: notas locais privadas (o método: tags livres transversais; anti-orçamento-por-categoria).
> GAP de feature + WRONG (categorias granulares = anti-padrão → rebaixar para tags).

## Problema
O método usa **tags livres** (cor + emoji), aplicáveis a qualquer lançamento, que somam por mês —
incluindo a tag especial "! Pagar" (a-pagar). O Neko não tem tabela `tag`. E tem uma **árvore
granular de `category`** que é o "orçamento por categoria" que o método rejeita — deve ser
rebaixada para tags, mantendo só `category.nature` (fixed/variable), que é legítimo.

## Modelo
- **`tag`**: `id`, `name`, `color` (token/hex), `emoji` (opcional), `is_special` (a tag "! Pagar"),
  `created_at`. Cota: limite brando configurável (default alto); a UI sinaliza ao se aproximar.
- **`transaction_tag`**: N:N (`transaction_id`, `tag_id`), PK composta; ON DELETE CASCADE.
- `category.nature` permanece (atributo do lançamento: fixo/variável). A **árvore** granular de
  `category` (parent_id) deixa de ser orçamento — vira fonte de tags (migração de dados futura;
  por ora, novas tags são de 1ª classe e a árvore fica dormente, sem UI de orçamento).

## Backend (determinístico, TDD)
- `create_tag(name, color, emoji?, is_special?)` → id.
- `list_tags()` → tags.
- `set_transaction_tags(transaction_id, tag_ids[])` → substitui as tags do lançamento (UPSERT do N:N).
- `tag_totals_for_month(year, month)` → por tag: soma dos valores dos lançamentos do mês com aquela
  tag (a "soma por tag" do método). "! Pagar" ordena no topo.

## UI (slice seguinte)
Tela/aba Tags: lista de tags coloridas com emoji + total do mês; filtro; criar/editar tag;
atribuir tags no formulário de lançamento. (Componentes DS já prontos.)

## DoD
- Migração `tag` + `transaction_tag` (idempotente; ON DELETE CASCADE).
- Comandos com testes de integração (criar, listar, atribuir, somar por mês; "! Pagar" no topo).
- Sem orçamento-por-categoria reintroduzido; `category.nature` preservado.
- `npm run check` verde.
