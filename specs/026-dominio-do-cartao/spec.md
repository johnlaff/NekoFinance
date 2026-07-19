# Spec 026 — Domínio do cartão: contas-cartão, faturas persistidas, séries e reembolso vinculado

## Contexto

O cartão de crédito é hoje um colapso: o schema de `account` já carrega
`credit_card`/`closing_day`/`due_day`/`credit_limit`/`linked_account_id`, mas
nenhum writer de produção cria essa conta (`create_account` deriva liquidez por
tipo e rejeita `credit_card`); nenhuma transação conhece o cartão a que
pertence; o write-back colapsa todo crédito num lump de UM cartão
(`ORDER BY created_at, id LIMIT 1` em `write_back_cmds.rs`, com
`multi_card_warning` paliativo); o import parseia as seções de nota
(`CARTÕES:`/`FATURAS:` → `ItemKind::Cartao`) mas persiste só o texto da seção
em `line_item.section` — a classificação não vira vínculo com conta nenhuma.

A fatura não existe como entidade: "próximo vencimento" é o primeiro dia com
eventos `Cartao` agregados por data, sem identidade de cartão
(`forecast_cmds.rs`). Assinaturas e parcelas de cartão não pré-lançam nas
faturas futuras. O reembolso existe só como marcador de nota
(`#reembolso:<quem>` → Entrada derivada `derived:reembolso:…`), sem vínculo
consultável com fatura ou compra.

Esta spec aterrissa o contrato selado na issue #179 e executado pela #195:
cartão = conta habilitada com aliases, fatura persistida cartão×ciclo,
`card_series` única para assinatura e parcelamento, reembolso = Entrada
vinculada, import por alias com proposta, write-back multi-cartão sob o
contrato célula×nota do #176, forecast por fatura e `card_gate` de 2 pernas
(emenda ao #178). Princípio de produto transversal: **guiar, nunca punir**.

## Substrato (fatos verificados no recon)

- `account`: campos de cartão completos; liquidez `NULL` p/ cartão (fora dos
  bolsos, `pockets.rs:34`); sem writer nem aliases.
- `transaction`: `payment_method='credit'` é o único traço de cartão; sem FK
  de fatura/série. IDs `derived:reembolso:<txn>:<line>` e `derived:dividir:…`
  nascem dos marcadores de nota no import (`import.rs:1229`) e são excluídos do
  write-back (`write_back_cmds.rs:15`).
- Import: célula com valor 0 nunca materializa `ImportedRow`
  (`import.rs:1379/1394/1409`) — é por isso que fatura futura de total zero
  não vira estrutura (P1 adiado do #192). A cerimônia do teto demonstra o
  padrão de varredura direta da grade de notas fora do checksum
  (`sheets_import.rs:225-242`) e o padrão proposta-por-identidade
  (`ceiling_proposal`, status `pending/accepted/dismissed`).
- Forecast: evento `Cartao` nasce de `payment_method='credit'`
  (`forecast/mod.rs:243`) ou da decomposição por item de nota
  (`forecast_cmds.rs:1217`); `cost_of_living = fixed_out + daily_realized +
  cartao` (`forecast/mod.rs:399`). `card_gate` atual tem 1 perna (economia
  anual ≥ 2000 bps, `forecast_cmds.rs:2106`); `reserve_months` e
  `reserve_state` já existem (`forecast_cmds.rs:2042-2064`).
- `cycle_due_date` (`forecast/mod.rs:286`) mapeia compra APÓS o fechamento
  para o ciclo que fechou no mês ANTERIOR — vencimento antes da própria compra
  (teste `cycle_due_date_after_closing`: compra 25/jan, fecha 20 → vence
  10/jan). Usada só pelo write-back; morre nesta spec.
- Write-back: lump de cartão = RAW sem nota (`items: None`,
  `write_back_cmds.rs:98`); a reconstrução de nota por seções já existe para
  células itemizadas (`write_back.rs:136-159`).
- UI: movimento `cartao` → `expense` + `paymentMethod:'credit'`
  (`movement.ts:19-33`), sem seletor de cartão; `PocketType` exclui
  `credit_card`; shell registra telas em `Screen`/`SCREEN_META`/`NAV_ITEMS`
  (+`DOCK_KEYS`/`MORE_KEYS`) em `AppShell.tsx:37-84`.

## D1 — Cartão = conta `credit_card` habilitada, com aliases

Writer próprio `create_card_account` (fora de pockets — cartão é passivo, não
bolso): `name`, `institution?`, `closing_day` (1–28, validado na fronteira),
`due_day` (1–31), `credit_limit_cents?`, `owner_person_id` (default "Eu"),
`aliases: Vec<String>`, `linked_account_id?` (D2). Editor `update_card_account`
para os mesmos campos.

Aliases vivem em tabela própria:

```sql
CREATE TABLE card_alias (
    id         TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    -- normalizado (casefold + accent-fold, sem ':' final) — mesma normalização
    -- de normalize_item_section/obligations
    alias      TEXT NOT NULL UNIQUE
);
```

O nome do cartão é implicitamente um alias (normalizado) — a tabela guarda os
extras. O casamento no import (D6) usa a descrição normalizada da linha da
seção de cartões.

Limite é **exibição discreta** na tela Cartões (rodapé do cartão, sem barra,
sem velocímetro, sem cor de status) — nunca insumo de régua. Milhas/pontos não
existem no modelo.

## D2 — Cartão adicional = conta-cartão vinculada

Adicional = conta `credit_card` com `owner_person_id` da outra pessoa e
`linked_account_id` apontando o titular. Ciclo **herdado**: `closing_day`/
`due_day` ficam `NULL` no adicional e toda derivação lê do titular (função
única `effective_cycle(account) -> (closing, due)`).

Sub-fatura 1:1: o adicional tem as próprias linhas de `invoice` por ciclo
(mesmo `cycle_month` do titular). Na planilha real cada dono já é uma linha da
nota — cada linha casa (por alias) com a conta correspondente e alimenta a
fatura DELA. O total do emissor = fatura do titular + Σ sub-faturas vinculadas
(derivado, exibido no drill; nunca persistido).

A parte do adicional acumula como lump digitável (ajuste direto do
`stated_total` da sub-fatura); itemizar compras é opcional. Dono-por-item sai
da relação compra → conta-cartão → `owner_person_id` — nunca é categoria.

## D3 — Fatura persistida

```sql
CREATE TABLE invoice (
    id           TEXT PRIMARY KEY NOT NULL,
    account_id   TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    -- identidade cartão×ciclo: "YYYY-MM" do MÊS DO VENCIMENTO
    cycle_month  TEXT NOT NULL,
    closing_date TEXT NOT NULL,
    due_date     TEXT NOT NULL,
    -- autoridade quando presente (import/ajuste manual); NULL = derivar da soma
    stated_total_cents INTEGER,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (account_id, cycle_month)
);
```

- **Status derivado, nunca armazenado** (função pura de `today` ×
  `closing_date` × `due_date`, sem drift):
  `prevista` (ciclo ainda não abriu: `today < closing_date_anterior + 1`) ·
  `aberta` (ciclo corrente: acumulando compras) · `fechada`
  (`closing_date < today <= due_date`) · `paga` (`due_date < today`).
- **`stated_total` é a autoridade** quando presente. Total efetivo =
  `stated_total` se houver, senão Σ compras. Divergência entre `stated_total`
  e Σ compras = **linha de reconciliação** no drill (D10) — nunca some, nunca
  vira item (leitura honesta: "parte não itemizada" da fatura).
- **Contabilidade aditiva** (evita dupla contagem entre planilha, compras do
  app e séries): quando `stated_total` está presente, todo gesto que muda as
  compras da fatura ajusta o `stated_total` na MESMA transação — registrar
  compra soma, apagar subtrai, remanejar subtrai na origem e soma no destino,
  materializar/regenerar/cancelar ocorrência de série idem. É o gesto do
  método ("a compra soma na fatura em aberto") aplicado ao total declarado.
  O ajuste direto (D10) grava valor absoluto. Com `stated_total` ausente, o
  efetivo deriva da soma e nada é ajustado.
- Datas explícitas por fatura: mudar o ciclo da conta depois não reescreve
  faturas existentes. Import fixa `due_date` = data da célula (autoridade da
  planilha); criação pelo app deriva do ciclo efetivo.
- Compras → fatura por FK: `ALTER TABLE "transaction" ADD COLUMN invoice_id
  TEXT REFERENCES invoice(id)`. Default = fatura do ciclo derivado da data da
  compra (D5); **remanejo manual permitido** (mover compra para fatura
  adjacente do mesmo cartão).
- Realização: a fatura `paga` corresponde à Saída realizada da célula do
  vencimento importada da planilha — casada por consulta (data + seção
  cartões + alias), nunca por FK a `line_item` (itens são re-derivados a cada
  import; mesma razão do resolver de `obligation`).

## D4 — Derivação de ciclo (núcleo puro, substitui `cycle_due_date`)

Duas funções puras em `forecast/mod.rs` (TDD primeiro):

- `cycle_close_for_purchase(purchase: NaiveDate, closing_day: u32) ->
  NaiveDate` — dia da compra ≤ `closing_day` (clamp 1..=28) → fecha no
  próprio mês; senão → fecha no mês SEGUINTE. (Corrige a inversão atual: a
  compra de 25/jan com fechamento 20 entra no ciclo que fecha 20/fev.)
- `due_date_for_close(close: NaiveDate, due_day: u32) -> NaiveDate` — primeira
  ocorrência de `due_day` estritamente APÓS o fechamento (mesmo mês quando
  `due_day > closing_day`, senão mês seguinte; clamp ao último dia do mês).

`cycle_month` = `"YYYY-MM"` do `due_date`. `cycle_due_date` morre; o único call
site de produção (lump do write-back) passa a agrupar por fatura (D7). Teste de
regressão documenta a semântica nova com os casos 15/jan e 25/jan.

## D5 — `card_series` única (assinatura e parcelamento)

```sql
CREATE TABLE card_series (
    id                TEXT PRIMARY KEY NOT NULL,
    account_id        TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    description       TEXT NOT NULL,
    amount_cents      INTEGER NOT NULL CHECK (amount_cents > 0),
    -- NULL = assinatura (infinita); N = parcelamento em N vezes
    count             INTEGER CHECK (count IS NULL OR count BETWEEN 1 AND 120),
    -- "YYYY-MM" da PRIMEIRA fatura (identidade de ancoragem)
    start_cycle_month TEXT NOT NULL,
    -- assinatura cancelada a partir desta fatura (exclusive); NULL = ativa
    canceled_from_cycle_month TEXT,
    created_at        TEXT NOT NULL DEFAULT (datetime('now'))
);
ALTER TABLE "transaction" ADD COLUMN card_series_id
    TEXT REFERENCES card_series(id) ON DELETE CASCADE;
```

- Ocorrência = compra projetada (`is_projection=1`) vinculada à série E à
  fatura, **uma por fatura consecutiva** (meses consecutivos de
  `cycle_month` a partir de `start_cycle_month`) — nunca por data solta.
- `n/N` **derivado do índice**: `n = meses(start_cycle_month → cycle_month) +
  1`; nunca persistido.
- Materialização em janela rolante: parcelamento materializa as N; assinatura
  materializa até o fim do ano-planilha corrente, no mínimo (re-materializada
  quando a janela avança). Materializar cria a fatura `prevista` que faltar.
- Editar (valor/descrição) **regenera as ocorrências restantes sob a mesma
  identidade** (compras de faturas `paga`/`fechada` intocadas); cancelar
  assinatura fixa `canceled_from_cycle_month` e apaga as ocorrências a partir
  dali. Tudo em transação única com rollback comprovado (pool de 1 conexão:
  derivar/ler ANTES do `begin` ou via `&mut *tx`).
- `card_series` é ortogonal a `recurrence` (série de data): compra de cartão
  recorrente usa `card_series`, nunca `recurrence`.

## D6 — Import por alias com proposta

A varredura de faturas segue o padrão da cerimônia do teto: **varredura direta
da grade de notas da coluna Saída**, fora do fluxo de linhas materializadas e
fora do checksum — assim célula projetada de valor total **zero** também
materializa estrutura (mata o P1 adiado do #192).

Para cada célula da coluna Saída com nota contendo seção de cartões
(`CARTÕES`/`CARTOES`/`FATURA`/`FATURAS`):

1. Cada linha `R$ <valor> - <descrição>` da seção casa a descrição normalizada
   contra `card_alias` (e nome normalizado da conta).
2. **Alias conhecido** → upsert de `invoice` por `(account_id, cycle_month da
   data da célula)` com `stated_total = valor da linha` e `due_date = data da
   célula`. Linha `R$ 0,00` em célula futura materializa a fatura `prevista`
   com `stated_total = 0`.
3. **Alias desconhecido** → `card_proposal` (padrão `ceiling_proposal`):

```sql
CREATE TABLE card_proposal (
    id           TEXT PRIMARY KEY NOT NULL,
    -- identidade: alias normalizado — o mesmo alias nunca re-propõe
    alias        TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    source_month TEXT NOT NULL,
    status       TEXT NOT NULL CHECK(status IN ('pending','accepted','dismissed')),
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    resolved_at  TEXT
);
```

   Aceite (UI, D10) pede fechamento/vencimento/dono e cria a conta + alias;
   dispensada nunca re-propõe. **Nenhuma conta nasce em silêncio.**
4. Ano anterior best-effort: linha sem valor parseável não vira fatura;
   célula realizada alimenta fatura `paga` (histórico).
5. O upsert respeita `stated_total` de ajuste local mais novo que o import
   (mesma régua de 3 vias do reconcile, com base própria:
   `invoice.source_stated_total_cents`, migração desta fatia): planilha mudou
   e local não → aplica planilha (total e base); só local mudou → preserva o
   local e avança a base; ambos mudaram e divergem → conflito na fila
   `import_conflict` (`transaction_id = "invoice:<id>"`, campo
   `stated_total`), resolvido pela UI existente de conflitos — e o write-back
   segue bloqueado até resolver (guarda existente).

O marcador `#reembolso:` continua criando a Entrada derivada; quando a linha
etiquetada casa um alias de cartão, a Entrada derivada ganha
`refund_invoice_id` da fatura daquela linha (D8) — o vínculo nasce no import
sem migração de backfill (derivadas são recomputadas a cada import).

## D7 — Write-back multi-cartão (o `LIMIT 1` morre)

O lump único vira **uma linha por cartão** na célula de Saída do dia de
vencimento, sob a seção de cartões, com merge cirúrgico:

- Candidatas de cartão = faturas com `due_date` no ano-alvo. Valor da linha =
  total efetivo da fatura (autoridade `stated_total`). Descrição da linha =
  alias primário/nome do cartão (preserva a convenção da nota do dono —
  round-trip com D6).
- A nota da célula é reconstruída com as seções não-cartão vindas dos
  `line_item` (mecânica existente de `write_back.rs:136-159`) + a seção de
  cartões vinda das faturas; o valor da célula = Σ de todas as linhas
  (fórmula `=SUM(...)` quando ≥ 2 partes, regra existente).
- A perna de Entrada do reembolso vinculado (D8) pré-lançada pelo app entra
  como candidata `Entrada` normal na data do vencimento (Entradas derivadas
  do import continuam excluídas — vieram da planilha).
- `multi_card_warning` morre (campo, computo e UI); a auditoria pós-escrita
  baseada em `cycle_due_date` (realinhamento de crédito) passa a casar por
  fatura.
- Sem NENHUM cartão configurado, o comportamento atual (crédito na data da
  compra) permanece.

## D8 — Reembolso = Entrada vinculada; réguas em bruto

```sql
ALTER TABLE "transaction" ADD COLUMN refund_invoice_id
    TEXT REFERENCES invoice(id) ON DELETE SET NULL;
ALTER TABLE "transaction" ADD COLUMN refund_txn_id
    TEXT REFERENCES "transaction"(id) ON DELETE SET NULL;
ALTER TABLE "transaction" ADD COLUMN refund_series_id
    TEXT REFERENCES card_series(id) ON DELETE SET NULL;
```

- Uma Entrada carrega **no máximo um** alvo (invariante validada na fronteira
  dos commands + testes; alvo: fatura — inclusive parcial —, compra ou série).
  Reembolso **nunca** reduz fatura, valor de compra ou régua.
- **Regime bruto canônico**: entrada cheia + saída cheia em TODAS as réguas
  julgadoras (custo de vida, economia, performance). O vínculo habilita
  somente a **lente líquida didática** ("custo de vida líquido de
  reembolsos") — leitura marcada no drill da fatura e popover, nunca a
  julgadora.
- Fluxo do protocolo do dono: sub-fatura do adicional gera a expectativa de
  reembolso = Entrada projetada no vencimento com `refund_invoice_id` da
  sub-fatura (as duas pernas pré-lançadas no futuro — critério de aceite).
- Série reembolsável (ex.: assinatura compartilhada): Entrada projetada por
  ocorrência com `refund_txn_id` da compra correspondente, ou única com
  `refund_series_id`.

## D9 — Forecast por fatura + `card_gate` de 2 pernas

**Fonte dos eventos `Cartao` com cartão configurado:**

- Faturas `aberta`/`fechada`/`prevista` → um evento `Cartao` por fatura no
  `due_date`, valor = total efetivo. Eventos `Cartao` crus (compra com
  `payment_method='credit'`, itens de seção cartões em células FUTURAS) são
  **suprimidos** quando a data pertence a uma fatura viva — a fatura é a
  única voz do futuro (sem dupla contagem entre projeção importada e fatura:
  a projeção importada É a fatura, via D6).
- Faturas `paga` → a realização importada da planilha continua sendo a
  autoridade (transações/itens realizados, comportamento atual).
- Sem cartão configurado → comportamento atual intocado (evento na data da
  compra).

**Check-in / DashboardSummary:** `cartao_month_cents` e
`next_fatura_date/amount` passam a derivar das faturas; novo campo
`upcoming_invoices: Vec<UpcomingInvoiceDto>` (`account_id`, `card_name`,
`due_date`, `amount_cents`, `status`) — vencimentos POR CARTÃO substituem o
ciclo único (insumo da Onda Hoje). O buraco do futuro enxerga as séries
pré-lançadas porque as ocorrências materializadas vivem nas faturas que geram
os eventos.

**`card_gate` de 2 pernas** (emenda ao #178; estados epistêmicos respeitados):

- Perna economia (existente): economia 20–30% viva (`>= 2000` bps).
- Perna reserva (nova): `reserve_months >= 6`, com `reserve_state` do #192 —
  `no_record` → perna `unknown` (nunca vira `below` fabricado).
- DTO: `card_gate` (veredito composto: `alive` só com as duas pernas vivas;
  `below` se qualquer perna computável falha; `unknown` se nenhuma perna é
  computável) + `card_gate_economy` e `card_gate_reserve` (por perna, mesmos
  três valores). A 3ª perna canônica ("sem pressa para o próximo objetivo
  patrimonial") é **didática**: copy no popover do gate, sem computo.

## D10 — Tela Cartões (sub-ledger) e registro

Nova tela `cartoes` no shell (Screen + SCREEN_META + NAV_ITEMS + MORE_KEYS;
posição definitiva decide na onda de identidade — provisória: sidebar após
"Este mês", menu "Mais" no mobile).

- **Lista**: um cartão por card — nome, dono, próximo vencimento (data +
  total efetivo + status), fatura aberta acumulando, limite discreto quando
  informado. Adicionais aninham sob o titular com o recorte por dono.
  Estados de dado ausente com as primitivas do #192 (`NoRecordDash` para
  cartão sem fatura, nunca zero fabricado).
- **Drill da fatura**: compras (data, descrição, valor, dono, `n/N` quando de
  série), séries ativas, reembolsos vinculados (com a perna de Entrada),
  **linha de reconciliação** quando `stated_total` ≠ Σ compras (aparência de
  linha sintética, mesma gramática do protótipo do #176 — nunca item
  editável), total por dono (titular × adicionais), lente líquida didática
  marcada.
- **Gestos de 1ª classe**: registrar compra (kind `cartao` no
  NewTransactionForm ganha seletor de cartão obrigatório quando existe > 1,
  default = último usado; série assinatura/parcela `N`; expectativa de
  reembolso) e **ajustar `stated_total` direto** na fatura.
- **Proposta de cartão** (D6): banner com aceite explícito (pede
  fechamento/vencimento/dono) e dispensa — padrão do banner do teto.
- **Gate "guiar, nunca punir"**: o estado do gate informa QUAL perna falta e
  mostra o caminho com a matemática do método (quanto falta de economia/
  reserva); com as pernas vivas, paleta de paz. Sem vermelho moralizante,
  badge de vergonha ou nag de migração; didática canônica completa (teste de
  2–3 meses no débito como CONVITE em popover). Critério de aceite do
  critique.

## Fronteiras

- Dono-por-item e vínculo de reembolso são metadados de fluxo — **nunca
  categorias**; tags seguem diagnósticas; **zero categorização de fatura**.
- Milhas/pontos/limite não são critérios de NADA (limite = exibição
  discreta).
- Mia, Onda Hoje (#187) e reposicionamento do shell ficam fora — esta spec
  entrega o substrato e a tela Cartões.

## Plano de execução (fatias, TDD no domínio)

- **A — Migrações + núcleo puro**: tabelas `invoice`/`card_series`/
  `card_alias`/`card_proposal`, colunas de `transaction`; `cycle_close_for_
  purchase`/`due_date_for_close`; status derivado; total efetivo;
  reconciliação. `cycle_due_date` morre.
- **B — Commands**: `create_card_account`/`update_card_account` (+ aliases,
  adicional), `list_cards`, registrar compra na fatura (default + remanejo),
  `set_invoice_stated_total`, reembolso vinculado (criar/desvincular), CRUD
  de `card_series` (materializar/editar/cancelar) — transações atômicas.
- **C — Import**: varredura de faturas na grade de notas (alias → upsert;
  zero futuro materializa; proposta p/ alias desconhecido; 3 vias p/
  `stated_total`; vínculo do `#reembolso` a fatura).
- **D — Write-back**: candidatas por fatura, linha por cartão, merge com
  seções não-cartão, perna de Entrada do reembolso; `multi_card_warning` e
  `LIMIT 1` morrem; auditoria pós-escrita por fatura.
- **E — Forecast + gate**: eventos por fatura com supressão de dupla
  contagem, `upcoming_invoices`, `card_gate` 2 pernas + DTOs.
- **F — UI**: tela Cartões (lista/drill/gestos/proposta/gate), seletor de
  cartão + série + reembolso no NewTransactionForm, tipos em `api.ts`.

Cada fatia com TDD no domínio (faturas, ciclos, séries, vínculos, import,
write-back, gate); regressão para o bug de `cycle_due_date`; atenção ao pool
SQLite de 1 conexão (ler antes do `begin`).

## Critérios de aceite (do ticket #195)

1. Lançamento de cartão carrega: cartão de destino, dono (via conta), vínculo
   de reembolso, série (assinatura/parcela `n/N`).
2. Fatura compartilhada mostra o total e o recorte por dono; o reembolso
   esperado aparece vinculado no vencimento (as duas pernas pré-lançadas no
   futuro).
3. Assinatura/parcela pré-lança nas faturas futuras e o buraco do futuro as
   enxerga (forecast `Cartao` deriva das faturas por cartão; vencimentos por
   cartão substituem o ciclo único).
4. Write-back mantém a planilha legível pela convenção atual das notas;
   `multi_card_warning` morre.
5. Import materializa fatura futura de total zero; alias desconhecido nunca
   cria conta em silêncio.
6. Tela Cartões com drill da fatura (compras, séries, reembolsos, linha de
   reconciliação); estados de dado ausente com as primitivas do #192.
7. Princípio "guiar, nunca punir" verificado no critique (tom, cores,
   hierarquia dos estados do gate).

## Gates

TDD no domínio; revisão adversarial multi-dimensional antes do PR; impeccable
audit+critique na tela nova; smoke visual Playwright; `npm run check` verde;
CHANGELOG em `[Unreleased]`.
