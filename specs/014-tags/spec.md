# Spec 014 — Tags (N:N, cor; emoji + fixar-no-topo são extras do Neko) + categorias→tags

> Fonte: notas locais privadas (o método: tags livres transversais; anti-orçamento-por-categoria).
> GAP de feature + WRONG (categorias granulares = anti-padrão → rebaixar para tags).

## Problema

O método usa **tags livres** (nome + cor), aplicáveis a qualquer lançamento, que somam por mês.
"! Pagar" é uma convenção de nome do usuário (o "!" já ordena no topo por ASCII). O Neko não tem
tabela `tag`. E tem uma **árvore granular de `category`** que é o "orçamento por categoria" que o
método rejeita — deve ser rebaixada para tags, mantendo só `category.nature` (fixed/variable), que
é legítimo.

> Afordâncias próprias do Neko (não fazem parte do método): `emoji` e `is_special` (fixa no topo,
> com negrito). São conveniências de UI, não atributos do modelo de tags do método.

## Modelo

- **`tag`**: `id`, `name`, `color` (token/hex), `emoji` (opcional, Neko), `is_special` (opcional,
  Neko — fixa no topo), `created_at`. Cota: limite brando configurável (default alto); a UI sinaliza
  ao se aproximar.
- **`transaction_tag`**: N:N (`transaction_id`, `tag_id`), PK composta; ON DELETE CASCADE.
- `category.nature` permanece (atributo do lançamento: fixo/variável). A **árvore** granular de
  `category` (parent_id) deixa de ser orçamento — vira fonte de tags (migração de dados futura;
  por ora, novas tags são de 1ª classe e a árvore fica dormente, sem UI de orçamento).

## Backend (determinístico, TDD)

- `create_tag(name, color, emoji?, is_special?)` → id.
- `list_tags()` → tags.
- `set_transaction_tags(transaction_id, tag_ids[])` → substitui as tags do lançamento (UPSERT do N:N).
- `tag_totals_for_month(year, month)` → por tag: soma dos valores dos lançamentos do mês com aquela
  tag (a "soma por tag" do método). Tags `is_special` (afordância do Neko) ordenam no topo.

## UI (slice seguinte)

Tela/aba Tags: lista de tags coloridas com emoji + total do mês; filtro; criar/editar tag;
atribuir tags no formulário de lançamento. (Componentes DS já prontos.)

## DoD

- Migração `tag` + `transaction_tag` (idempotente; ON DELETE CASCADE).
- Comandos com testes de integração (criar, listar, atribuir, somar por mês; `is_special` no topo).
- Sem orçamento-por-categoria reintroduzido; `category.nature` preservado.
- `npm run check` verde.
