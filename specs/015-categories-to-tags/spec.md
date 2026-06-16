# Spec 015 — Categorias → Tags (rebaixar o orçamento-por-categoria)

> Fonte: notas locais privadas (o método rejeita orçamento-por-categoria). WRONG #4 (cirúrgico).

## Problema

O seed de `category` criava uma **árvore granular** (Moradia, Transporte, Saúde, Alimentação,
Lazer, Vestuário, Cartão Adicional…) — a estrutura de **"orçamento por categoria"** que o método
explicitamente rejeita. Estava **dormente**: nenhum comando ou tela do app a usava para orçamento
(só aparecia em testes de schema e nos bundles de referência do DS).

## Decisão

- A classificação **fina** do usuário passa a ser **TAGS** (spec 014) — livres, com cor/emoji,
  transversais, que somam por mês.
- De `category`, permanecem só as **duas macro-naturezas** (`fixed`/`variable`) + "Sem categoria".
  A natureza fixo/variável já vem do **tipo de movimento** (Saída→fixo, Diário→variável); `category.nature`
  é preservado como conceito, mas não vira árvore de orçamento.
- Migração `20240612000004_demote_category_tree.sql` remove os 8 nós granulares (seguro: dormentes).

## Não-objetivos

- Não introduzir UI de orçamento por categoria (anti-padrão).
- Não remover `category.nature` nem a tabela `category` (a nature é legítima).

## DoD

- Migração de rebaixamento + teste de schema atualizado (3 categorias restantes; granular ausente).
- Tags como classificação fina (spec 014, já entregue).
- `npm run check` verde.
