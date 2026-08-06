# Spec 032 — Onda Tags: interruptores de contabilidade por régua

## Contexto

A tela Tags mostra "Gasto por tag" como manchete — exatamente a análise por categoria
que o método rejeita. A direção da identidade redefine a tela: **a tag é um interruptor
de contabilidade** — ela decide em quais réguas do método um lançamento conta — e a tela
é **o painel que protege a honestidade das réguas**. A ordem narra pela prova: veredito
(custo de vida) → dinheiro de terceiros (o app **detectou**) → exceções (você
**declarou**) → movimentação por rótulo (consequência, no fim, atrás de disclosure).

O motor hoje tem **um** booleano por tag (`exclude_from_totals`) que tira o lançamento
inteiro de todas as métricas de uma vez. O desenho exige **quatro interruptores
independentes** (Performance · Custo de vida · Economia · Diário médio) com o **Saldo
intocável** — e o efeito de cada interruptor exibido em reais, computado pelo motor
(nunca prosa estimada: Performance mexe pelo líquido, Custo de vida pela saída — repetir
o mesmo número seria mentira aritmética).

Alcance: **mês + anual em lockstep** — os quatro flags governam a tela mensal E os
agregados anuais (guardrail da economia, visões do ano). Terceiros: **agregação completa
sobre os vínculos que já existem** (marcadores de nota, splits, reembolsos de cartão,
donos de cartão adicional) — sem entidade nova.

## Modelo de domínio

### Schema — flags por régua

Migração `20260723000002_tag_ruler_flags.sql`:

```sql
ALTER TABLE tag ADD COLUMN exclude_from_performance    INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tag ADD COLUMN exclude_from_cost_of_living INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tag ADD COLUMN exclude_from_savings        INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tag ADD COLUMN exclude_from_daily_avg      INTEGER NOT NULL DEFAULT 0;
UPDATE tag SET exclude_from_performance    = exclude_from_totals,
               exclude_from_cost_of_living = exclude_from_totals,
               exclude_from_savings        = exclude_from_totals,
               exclude_from_daily_avg      = exclude_from_totals;
ALTER TABLE tag DROP COLUMN exclude_from_totals;
```

O flag antigo significava "fora de tudo" → backfill liga os quatro. A coluna antiga
morre (nenhum caminho de escrita externo depende dela; o modelo de tags é local do
Neko). **Não existe flag de Saldo** — o Saldo intocável é garantia estrutural, não
configuração (decisão 2 do desenho).

### Núcleo puro — máscara por régua

`forecast/mod.rs` ganha:

```rust
pub struct RulerMask { pub performance: bool, pub cost_of_living: bool,
                       pub savings: bool, pub daily_avg: bool }   // Copy; ALL = tudo true
pub struct MetricEvent { pub event: CashflowEvent, pub mask: RulerMask }
```

- `month_metrics`/`month_metrics_for`/`project_with_metrics` passam a receber
  `&[MetricEvent]` no stream de MÉTRICAS. O stream de CAIXA (`chain_events`)
  permanece `&[CashflowEvent]` — o Saldo não tem máscara por definição.
- A máscara de um evento = AND dos flags das tags do lançamento-pai (lançamento sem
  tag → ALL). Itens de nota e o resíduo da célula herdam a máscara do pai (mesma
  semântica de transação-inteira de hoje).
- Eventos sintéticos (teto projetado, fatura materializada, hipotéticos de cenário)
  → ALL.
- **A troca de tipo é o lockstep**: todo call site de métricas é forçado pelo
  compilador a declarar sua máscara — a simetria deixa de viver em comentário.

### `month_metrics` — acumuladores por régua

Os buckets alimentam réguas cruzadas, então cada régua acumula sua própria view:

| Régua         | Insumos (filtrados pela sua máscara)                                                        |
| ------------- | ------------------------------------------------------------------------------------------- |
| Performance   | `income_p − (fixed_p + daily_real_p + daily_proj_p + cartao_p + economia_p + patrimonio_p)` |
| Custo de vida | `fixed_c + daily_real_c + cartao_c`                                                         |
| Economia      | `economia_s × 10000 / income_s`                                                             |
| Diário médio  | `daily_real_d / dias decorridos`                                                            |

Campos do `MonthMetric` (cada um declara a view que serve — as equações exibidas nas
telas fecham com o motor, regra 6 do ui-standards):

- `performance_cents` — view Performance.
- `cost_of_living_cents`, `fixed_out_cents`, `daily_out_cents`, `cartao_cents` — view
  Custo de vida (componentes exibidos em Este mês).
- `income_cents` — view Economia (é a "sua renda" do método: denominador do
  Economizado% exibido; "o que a Gio devolve não entra na sua renda").
- **Novo** `income_performance_cents` — view Performance (para equação de Performance
  exibida fechar quando as views divergem).
- **Novo** `daily_avg_out_cents` — view Diário (numerador do Diário médio exibido).
- `economia_cents` — view Economia, reconciliada com a anotação da aba
  (`max(derivada, anotação)`).
- `daily_projected_cents`, `patrimonio_cents` — view Performance.
- `savings_rate_bps` — `economia_s / income_s`.
- `real_daily_avg_cents` — `daily_avg_out / decorridos`.
- `total_outflow_cents` — view Custo de vida + diário projetado (cobertura de meses:
  "quanto do viver já está lançado").

A reconciliação com a anotação aplica-se por view (`economia_p` e `economia_s`
reconciliam separadamente). Fronteira documentada: a anotação da aba não tem tag —
se ela dominar o `max`, desligar Economia de uma tag não muda o número, e o efeito
computado honestamente mostra R$ 0.

**Garantia de regressão**: com todas as máscaras ALL, cada número é idêntico ao motor
atual; com uma tag fora das 4 réguas, idêntico ao comportamento atual do flag único.
Testes golden cobrem os dois extremos + views divergentes.

### Sítios SQL em lockstep (cada agregado adota o flag da régua que alimenta)

| Sítio (`forecast_cmds.rs`)                                        | Flag                                                                                                                                         |
| ----------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `load_db_events` (caminho de métricas)                            | máscara por evento (mapa `transaction_id → 4 MINs` em query própria; caminho de caixa segue sem filtro)                                      |
| `realized_annual_savings` — perna renda                           | `exclude_from_savings` (base do guardrail 20–30%)                                                                                            |
| `realized_annual_savings` — perna net (colchão)                   | `exclude_from_performance` (net do ano = figura de performance)                                                                              |
| `realized_annual_economia` (itens + transfers)                    | `exclude_from_savings`                                                                                                                       |
| `realized_annual_patrimonio`                                      | `exclude_from_performance`                                                                                                                   |
| `projected_annual_savings` — renda / net                          | savings / performance (mesmo racional)                                                                                                       |
| `realized_savings_baseline` (medianas do gate de financiamento)   | `exclude_from_savings`                                                                                                                       |
| `realized_monthly_baseline` (mediana do custo de vida)            | `exclude_from_cost_of_living`                                                                                                                |
| `prev_month_daily_avg` (base do teto do dia)                      | `exclude_from_daily_avg` (mesma régua Diário médio do motor mensal/anual)                                                                    |
| `daily_spend`/`daily_spend_today` (numerador de "Diário de hoje") | `exclude_from_daily_avg`                                                                                                                     |
| `spending_mode_summary`                                           | view Custo de vida (detecção de modo é pergunta de forma-do-gasto; preserva o comportamento do backfill)                                     |
| Cobertura de dias do teto projetado (`days_with_daily`)           | **sem máscara** — fato comportamental (o dia teve registro), não valor de régua; um dia coberto por gasto excluído não recebe dupla projeção |
| `month_grid`, caminhos de caixa                                   | sem filtro (inalterados)                                                                                                                     |

### Vínculo de pessoa nos derivados

Migração `20260723000003_transaction_counterparty.sql`:

```sql
ALTER TABLE "transaction" ADD COLUMN counterparty_person_id TEXT REFERENCES person(id);
UPDATE "transaction" SET counterparty_person_id = (
  SELECT p.id FROM person p
  WHERE LOWER(p.name) = LOWER(TRIM(substr(description, instr(description, ':') + 1))))
WHERE id LIKE 'derived:reembolso:%' OR id LIKE 'derived:dividir:%';
```

O import já resolve/cria a pessoa ao processar `#reembolso:`/`#dividir:` mas só grava
o vínculo no `split`; a Entrada derivada carregava a pessoa apenas na descrição gerada
("Reembolso: {nome}"). A coluna torna o vínculo estrutural (o formato da descrição é
determinístico do próprio import, então o backfill em SQL puro é seguro); o import
passa a gravar `counterparty_person_id` nos dois INSERTs derivados. Sem entidade nova —
`person` já existe (decisão 3 do desenho).

## Dinheiro de terceiros — agregação

Fontes estruturais (todas já produzidas pelo import/domínio do cartão):

1. **Marcadores de nota** — Entradas derivadas com `counterparty_person_id`
   (`derived:reembolso:*` = valor integral da linha; `derived:dividir:*` = parte).
   A perna "saiu" = valor da linha marcada (reembolso) / share do `split` (dividir).
2. **Splits** — `split.owner_person_id` (parte de terceiro numa saída).
3. **Cartão adicional** — conta vinculada (`linked_account_id NOT NULL`) com
   `owner_person_id` próprio: "saiu" = total efetivo das sub-faturas da pessoa no
   ciclo; "voltou" = Entradas vinculadas (`refund_invoice_id` → faturas da conta dela)
   realizadas.
4. **Expectativas de reembolso** — Entrada vinculada com `is_projection = 1` (ou data
   futura) = retorno esperado, ainda não realizado.

Por pessoa, no mês da tela: `out_cents` (saiu), `back_cents` (voltou, realizado),
`expected_cents` (vinculado não realizado). Estados (view-model, dados do DTO):

- **favor** — voltou > saiu ("a seu favor").
- **open** — retorno esperado não realizado; idade = dias desde a saída/expectativa
  ("em aberto há N dias"). Abertos de meses anteriores continuam aparecendo (dívida
  não expira na virada); os fluxos exibidos são do mês da tela.
- **series** — reembolso vinculado a `card_series` com `count`: "parcela k de N" /
  "falta N parcelas".
- **settled** — saiu = voltou realizados ("quitado em {data}").
- **none** — pessoa conhecida sem lançamento no mês ("sem registro").

O dono do app nunca vira linha: pessoas entram pela posse de vínculo de terceiro
(split, derivado, conta **vinculada**) — a conta principal do titular não gera linha.

**Média mensal da manchete B** ("R$ N por mês, em média, é movimentação de …"):
média do `out` de terceiros nos últimos 12 meses completos + corrente, sobre os meses
com movimento detectado; sempre com selo `EstimateMark`; `people_count` = pessoas
distintas na janela. Zero detecção → manchete C.

## DTO da tela — `get_tags_screen(year, month)`

Módulo novo `src-tauri/src/tags_screen.rs` (núcleo puro de efeitos + comando fino).

```
TagsScreenDto {
  month: "YYYY-MM",
  verdict: { cost_current_cents,          // manchete A/C — custo de vida com os interruptores atuais
             cost_all_on_cents,           // "sem as exceções, contariam …"
             third_party_avg_cents: i64|null, third_party_people: u32,  // manchete B (estimativa)
             has_exceptions: bool },
  third_parties: [ { person_id, name, out_cents, back_cents, expected_cents,
                     state: "favor"|"open"|"series"|"settled"|"none",
                     open_since_days: u32|null, series_done: u32|null, series_total: u32|null,
                     settled_date: "YYYY-MM-DD"|null } ],
  tags: [ { id, name, color, emoji, is_special,
            counts_in: { performance, cost_of_living, savings, daily_avg },  // bool — true = Calcular
            month_total_cents,            // o que a tag movimentou (semântica atual do tag_totals)
            txn_count,
            effects: { performance_delta_cents,     // contribuição marginal (líquido)
                       cost_delta_cents,            // saída
                       savings_base_delta_cents,    // Δ na renda-base
                       savings_amount_delta_cents,  // Δ na economia registrada
                       daily_avg_delta_cents } } ],
  sync_stale_at: string|null              // manchete F — timestamp quando a leitura falhou
}
```

**Efeitos = contribuição marginal ao estado atual**: para a tag T e a régua R, o motor
recomputa o mês com o flag T×R invertido (demais interruptores como estão) e reporta a
diferença — o mesmo número serve à frase do estado ligado e desligado. Computado pelo
núcleo puro sobre os eventos do mês com os conjuntos de tags por lançamento (4×N
reexecuções de função pura — barato). A anotação real entra na reexecução, então o
efeito respeita a fronteira do `max` (pode ser honestamente R$ 0).

Exceção × rótulo é **derivado, não estado**: tag com algum interruptor desligado lista
em Exceções; com os quatro ligados, em Movimentação por rótulo. As duas listas saem do
mesmo `tags[]`.

### API de escrita

- `update_tag_rulers_cmd(tag_id, exclude_from_performance, exclude_from_cost_of_living,
exclude_from_savings, exclude_from_daily_avg)` substitui `update_tag_exclude_cmd`
  (UPDATE único, idempotente). `create_tag`/`update_tag`/`list_tags` ganham/expõem os
  4 flags; `exclude_from_totals` sai dos DTOs e do frontend.

## Estrutura da tela (ordem do DOM = ordem de leitura)

1. **Veredito** (`data-large-title`): vlabel "Custo de vida · {mês}" + `h1` com o
   número atual + frase de apoio por estado (abaixo). CTA só na manchete B.
2. **Dinheiro de terceiros** (sec-head ícone pessoas · nota inerte "detectado no
   import"): linhas pessoa (avatar-iniciais, nome, detalhe "Saiu X · voltou Y",
   valor+estado à direita). Seção some quando não há pessoa conhecida.
3. **Exceções** (sec-head ícone sliders · botão "Nova tag"): linhas de tag expansíveis
   (`details`) — chip da tag + resumo "fora de N de 4 réguas · M lançamentos" + valor.
   Expandida: 4 interruptores (`Switch` do DS), cada um com a frase fixa do que a régua
   mede + o efeito em reais quando desligado (`is-off` com peso); ação "Editar tag"
   (nome/emoji/cor — capacidade preservada da tela atual). Rodapé do card: garantia
   única "**O saldo da conta sempre conta** — …" (uma por seção, não por tag).
4. **Movimentação por rótulo** (`details.fold` fechado): linhas chip + `Meter` neutro
   (fração do maior rótulo, sem cor de status, sem % do total — a relação é N:N e as
   partes não somam o todo) + valor + contagem. Linhas expansíveis com o mesmo painel
   de interruptores (é o caminho de um rótulo virar exceção).
5. Navegação de mês: crumb vivo no shell (padrão da onda Lançamentos) + controle na
   appbar mobile.

### Estados da manchete (A–F do desenho, `tagsView.ts` puro)

| Estado | Condição                          | Manchete                                                                                                     |
| ------ | --------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| A      | `has_exceptions`                  | número atual + "já deixa de fora R$ X" + cauda "Sem as exceções, contariam R$ Y"                             |
| B      | sem exceção, terceiros detectados | "Suas réguas contam dinheiro que não é seu." + média mensal com `EstimateMark` + CTA "Tirar isso das réguas" |
| C      | sem exceção, sem detecção         | número seco + "Nenhuma exceção declarada — e nada a declarar." (sem parabéns)                                |
| D      | zero tags                         | vazio que ensina o conceito ("Tags não são categorias.") + CTA "Criar primeira tag"                          |
| E      | carregando                        | `EmptyState` skeleton (nunca spinner sobre conteúdo)                                                         |
| F      | sincronia falhou                  | número fica, com a idade (`sync_stale_at`) + "Tentar de novo"                                                |

Frases dos interruptores (metade fixa ensina o vocabulário; metade variável é o efeito):

- Performance mede "Quanto sobrou no mês." · efeito pelo **líquido** (pode ser negativo
  a favor: "ela devolve mais do que gasta"); líquido zero → "entrou e saiu: o resultado
  não muda".
- Custo de vida mede "Quanto você gasta para viver por mês." · efeito pela **saída**.
- Economia mede "Quanto você guarda da sua renda." · efeito pela **base** (renda) ou
  pela economia registrada, conforme o Δ dominante.
- Diário médio mede "Quanto você pode gastar por dia." · sem número (copy do desenho).

Efeito R$ 0 em régua desligada → só a frase fixa + "não muda o resultado" (nunca
"R$ 0,00 a menos").

## Regras do DS que nascem/valem aqui

- **Tag é sempre chip** (fundo tonal `color-mix` 17%/34% da cor `--cat-*`); **tipo e
  bolsão são sempre glifo** — a colisão de paleta resolve por canal, nunca subtraindo
  cores.
- Sem donut, sem % do total, sem cor de status, sem delta-seta e sem adjetivo no
  ranking de rótulos (decisões 7–8 do desenho).
- Dinheiro tabular nunca anima; os valores em reais trocam secos no gesto.
- `Switch` do DS (off-state corrigido); `Meter` para toda barra; `EmptyState` para
  carregando/erro; `EstimateMark` no número estimado.

## Desvios do protótipo (com porquê)

1. **Linhas de rótulo expansíveis** (protótipo: estáticas): sem isso não existe caminho
   para um rótulo virar exceção; mesmo painel de interruptores em toda tag.
2. **Criar/editar tag preservados** (nome, emoji, cor — capacidade da tela atual que o
   protótipo não cobre); editor na gramática da direção, cores mapeando `--cat-*`.
3. **Raio e espaçamento pelo contrato de tokens** (`--radius-md`, escala 4px), não os
   px do protótipo (regra 12).
4. **Seção de terceiros mostra o que os vínculos reais sustentam**: estados
   "parcela/em aberto" só quando derivam de expectativa/série vinculada — nunca
   fabricados.

## A11y e motion

- Interruptores `role="switch"` + `aria-checked`; nome acessível ESTÁVEL
  ("{Régua} · tag {nome}") — quem anuncia estado é o `aria-checked`.
- `details/summary` com foco visível; chevron 180ms `--ease-standard`;
  reduced-motion cobre `animation` E `transition`.
- Roving/foco: sem grid — tab linear segue o DOM. Alvos ≥ 44px no mobile por
  pseudo-elemento (padrão do `Switch`).
- Skeleton `role="status"`; erro `role="alert"`; contraste AA nos dois temas × 6
  paletas (thumb desligado ≥ 3:1 — herdado do fix do DS).

## Testes e gates

- **TDD Rust**: golden de equivalência (máscaras ALL ≡ motor atual; tag 4×off ≡ flag
  único atual); views divergentes por régua (income_s ≠ income_p etc.); fronteira da
  anotação; efeitos marginais (Gio real: perf −900,00 × custo −4.077,64); agregação de
  terceiros por fonte (marcador, split, cartão adicional, expectativa) e estados;
  backfill idempotente; **regressão com pool de 1 conexão** (deadlock conhecido).
- **TDD frontend**: `tagsView.test.ts` (6 manchetes, frases por régua e sinal, resumo
  N de 4, agrupamento exceção×rótulo); `TagsScreen.test.tsx` (DOM order, switches,
  estados).
- **e2e**: `Tags-{dark,light}` + `mobile-tags-{dark,light}` do zero (2×); fixture do
  `tauri-mock` com os três blocos povoados — inspecionar TODOS os baselines re-gravados
  (fixture compartilhada, regra 30).
- Gates da onda: `npm run check`, React Doctor sem novos, impeccable audit + critique,
  revisão adversarial multi-lente antes do PR, CI verde.
