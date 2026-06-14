# Spec 016 — Recorrências/séries

> Fonte: notas locais privadas (Repetir {Nunca, Diariamente, Semanalmente, Mensalmente}; editar
> "deste ponto" vs "toda a série"). GAP de feature.

## Modelo

- `recurrence` (id, frequency [diaria/semanal/mensal], infinite, repetitions, start_date).
- `transaction.recurrence_id` (NULL = avulso). Uma série gera N ocorrências (transações projetadas)
  compartilhando o `recurrence_id`; ids determinísticos `{rec_id}:{i}`.

## Core (puro, TDD)

- `occurrence_dates(start, freq, count)`: Diária (+1d), Semanal (+7d), Mensal (+1 mês com **clamp**
  do dia ao último válido — 31/jan + 1 mês = 28/fev).

## Shell (comandos)

- `create_recurring_series(template, freq, repetitions)` → recurrence_id (insere a série + N ocorrências).
- `delete_series_from(transaction_id)` → apaga a ocorrência e todas as posteriores ("deste ponto").
- `delete_series_all(recurrence_id)` → apaga toda a série + a linha `recurrence`.

## Fora de escopo desta slice (follow-up)

- `update_series_from/all` (editar deste-ponto/toda-série) — análogo ao delete, via UPDATE.
- `infinite` (a perder de vista) — hoje sempre `repetitions` finito; gerar janela rolante depois.
- UI: escolher Repetir no form de lançamento + ações "editar/apagar série".

## DoD

- Migração + core `occurrence_dates` testado (3 frequências + clamp).
- create/delete-from/delete-all com testes de integração.
- `npm run check` verde (clippy all-targets incluído).
