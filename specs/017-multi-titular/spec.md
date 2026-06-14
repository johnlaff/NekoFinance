# Spec 017 — Multi-titular (splits): quem é responsável por cada parte de um lançamento

> Fonte: notas locais privadas. Diferencial do Neko: suportar múltiplos pagadores/responsáveis por
> um mesmo lançamento (vários titulares), sem deixar de ser método-fiel no resto. Read-side primeiro
> (exibição); o write-side de criação de split é slice posterior.

## Problema

Um lançamento pode ter mais de um responsável (ex.: uma despesa dividida entre titulares de uma
conta conjunta). O método não modela isso; é uma extensão do Neko. Precisamos representar e exibir
"quem pagou/qual fatia" sem contaminar as regras financeiras do método (custo de vida, performance,
diário continuam por lançamento).

## Modelo

- **`split`**: `id`, `transaction_id` (FK, ON DELETE CASCADE), `amount` (centavos, magnitude),
  `owner_person_id` (FK), `note?`. Um lançamento sem split é de titular único (o caminho comum).
- A identidade estável do import (spec 012) ancora o split: re-import normal preserva o vínculo.

## Backend (determinístico, TDD)

- `splits_for_transaction(transaction_id)` → as partes de um lançamento (com o nome do titular).
- `owner_totals_for_month(year, month)` → soma por titular no mês (para a visão de quem-paga-o-quê).

## UI

Surge como `OwnerChip` no `TransactionRow` e em telas de detalhe: ponto colorido por titular +
rótulo. Ownership explícito é mandato do design system.

## DoD

- Tabelas/queries com testes de integração (listar splits, somar por titular/mês).
- Read-side não altera nenhuma regra do método; lançamentos sem split continuam de titular único.
- `npm run check` verde.
