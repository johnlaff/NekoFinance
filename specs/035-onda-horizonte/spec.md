# Spec 035 — Onda Horizonte: o radar do caixa

## Contexto

O Horizonte era uma tela de **gráfico + cartões de fim de mês + lista de vencimentos**:
um banner de status ("Seu saldo chega a R$ X em DD/mês"), a `BalanceTrajectory` genérica,
uma grade de saldos de fim de mês e a lista dos vencimentos dos próximos 60 dias. Cumpria
a função de "para onde o saldo vai", mas com a voz de um relatório, não de um veredito.

A direção **Conversa com a Mia** redefine a leitura: o Horizonte é **a única tela que olha
só para a frente** — previsto · em meses · até o fim dos dados — e responde à pergunta que o
método manda fazer: _tem buraco na estrada?_ Delimitação com as vizinhas, selada no desenho
([#202](https://github.com/johnlaff/NekoFinance/issues/202)):

- **O ano julga o método** (Economizado%); **Horizonte guarda o caixa** (o caminho, onde
  aperta, o que está comprometido); **Hoje mostra o "pode gastar" — o Horizonte é a prova
  dele** (o menor ponto do horizonte é o que o guardrail usa).

A honestidade epistêmica do [#178](https://github.com/johnlaff/NekoFinance/issues/178) vem
em três camadas: o que tem **lastro** ganha semáforo; o que não tem vira **"Conferir"** (sem
cor de aprovação); o que não existe é **"Sem registro"**.

A composição — estado do veredito, a geometria da estrada, os estados epistêmicos da grade,
o agrupamento dos compromissos — vive num **view-model puro** (`src/screens/horizonteView.ts`)
com TDD, no padrão das telas irmãs (`hojeView`, `anoView`, `tetoView`, `tagsView`). Nenhuma
regra de método nasce aqui: **a régua de lastro, o gasto típico, a fronteira e o traçado
"se custar o de sempre" vêm todos do motor** (`Forecast`) — o frontend só compõe.

**Zero mudanças de backend.** O `Forecast` já expõe `baseline_outflow_cents` (gasto típico),
`coverage[]` (cobertura por mês futuro), `trusted_through_month` (a fronteira do lastro),
`estimated_missing_cents` (o custo de fechar cada mês incompleto), `deepest_deficit` (o
buraco), `daily[]`, `month_end[]` e `months[]`. Os compromissos por mês (Card 4) leem os
lançamentos **projetados** de cada mês futuro por `get_recent_transactions` escopado ao mês
(`getMonthTransactions`) — que já traz `installment_index/total`, `has_refund_link` e as
seções da nota.

## Estrutura da tela (ordem do DOM = ordem de leitura)

1. **Veredito-herói** — máquina de três vozes (livre · aperto · vazio), rótulo de recorte
   ("Horizonte · Hoje → 31 de dezembro"), a frase que declara o menor ponto e o gêmeo
   honesto, e a **proveniência em mono** ("Lançado até 31/12/2026 · Planilha lida às HH:MM").
2. **A estrada até dezembro** (card) — o gráfico próprio: a linha do **lançado**, a **zona
   sem lastro**, o traçado pontilhado **"se custar o de sempre"**, o **zero** e o **menor
   ponto** marcados; legenda, os dois fins de ano (lançado × típico), a nota do lastro
   (regra dos 60%, o gasto típico) e o fold **"Ver a estrada em números"** (tabela).
3. **Os próximos 12 meses** (card, assinatura) — grade do mês atual + 11, cada mês com o
   estado epistêmico (vivido · previsto com lastro · sem lastro/Conferir · sem registro),
   o saldo no fim do mês e o link que **abre o mês no Calendário**.
4. **O que já está marcado** (card) — os compromissos **projetados** de agosto a dezembro,
   agrupados por mês (`<details>`, o próximo mês aberto): dia, descrição, subtítulo derivado
   (parcela `n/N` em mono, **reembolso = Entrada vinculada**, conta fixa/diário), valor com
   sinal. Cabeçalho com entra/sai do mês; resumo total (entra · sai · dias com lançamento).
5. **E se?** (card) — a entrada dos Cenários com as **duas réguas do gate** na copy (reserva
   ≥ 6 meses? economia de 20–30% viva?); reusa o side-sheet de cenários existente. Cenário
   ativo entra como **camada declarada** (barra acima da estrada), nunca silenciosa.

No mobile a leitura é uma coluna só, na ordem do DOM. No desktop os cards descem para um
**bento de duas colunas independentes** a partir de 900px (a régua de ambiente ganha do
desenho aprovado — coluna única deixaria a metade direita vazia), dissolvidas em
`display: contents` no mobile. O veredito e a estrada ocupam largura cheia.

## O veredito (três vozes epistêmicas)

O `horizonteView` seleciona o estado a partir do `Forecast`:

1. **Livre** (estado dos dados de referência) — o lançado nunca fica negativo. "Caminho livre
   até o fim de {mês de confiança}." + o menor ponto à vista (com a data) + o **gêmeo
   honesto**: se os meses sem lastro custarem o gasto típico, onde dezembro termina — e,
   quando isso raspa o zero, o valor com o selo `EstimateMark`.
2. **Aperto** — o lançado cruza o zero (`deepest_deficit != null`). "O caminho aperta em
   {mês}." + o buraco com **data, tamanho e três saídas** (antecipar entrada · adiar saída ·
   cruzar com a reserva por partes) — guia, nunca acusa. CTAs "Simular uma saída" / "Abrir
   {mês}".
3. **Vazio** — não há futuro lançado suficiente para uma estrada (`baseline_outflow_cents`
   é 0 ou não há mês futuro com movimento). "O radar só enxerga o que está lançado." + como
   a estrada nasce (pré-lançar contas fixas, parcelas, faturas, salário) + CTA "Pré-lançar o
   futuro".
4. **Carregando** — `EmptyState` variante esqueleto no lugar do veredito e da estrada (nunca
   spinner sobre conteúdo, nunca `R$ 0,00` fabricado).

## A estrada (geometria derivada do motor)

- **Linha do lançado**: `daily[].balance_cents` de hoje ao fim do horizonte.
- **Fronteira do lastro**: o primeiro dia cujo mês é posterior a `trusted_through_month` — a
  partir dele a zona ganha o tom `--lift` e a linha vira tracejada ("Lançado, sem lastro").
- **Traçado "se custar o de sempre"**: para cada fim de mês futuro, `saldo lançado − Σ
estimated_missing_cents` dos meses sem lastro até ele (o mesmo `estimated_missing_cents`
  que O ano usa para `DEZ_TÍPICO`). Pontilhado.
- **Menor ponto**: o mínimo de `daily[].balance_cents` (marca + rótulo).
- **Zero** e **rótulos de mês** (1º dia de cada mês) desenhados.
- Os dois fins de ano: **lançado** (`month_end` de dezembro) × **típico** (o traçado acima),
  o típico com selo de estimativa quando difere.

A régua dos 60% e o gasto típico saem de `baseline_outflow_cents` e `coverage[]`
(`coverage_bps`) — a tela **declara** a régua, não a recalcula.

## A grade dos 12 meses (assinatura)

Mês corrente + 11. O estado epistêmico de cada mês deriva de `today`, `trusted_through_month`
e `month_end`/`months`:

- **Vivido** (mês corrente, dias já passados) — banda cheia do saldo.
- **Previsto com lastro** (mês futuro ≤ fronteira) — banda em opacidade reduzida.
- **Sem lastro · Conferir** (mês futuro > fronteira, com dado) — tracejado, **sem cor de
  aprovação**; selo "Conferir".
- **Sem registro** (mês sem `month_end`) — vazio, tracejado tênue, não navegável.

A **cor da banda** de um mês vem de `saldoBand(saldo de fim de mês)` — as faixas absolutas da
planilha (`lib/saldoHeatmap`). Isso satisfaz o dado real (tudo folga → verde) e o
follow-up do desenho: um mês sintético apertado/negativo pinta âmbar/vermelho na grade. A
verdade dia-a-dia mora no Calendário, que cada mês abre. Cada mês (com dado) é um link
`aria-label`ado que navega ao Calendário do mês.

## Os compromissos (Card 4) — composição honesta

Os compromissos são os lançamentos **projetados** que o motor já usa para montar a estrada.
O view-model agrupa por mês futuro e, por item, deriva o subtítulo dos campos reais
(`installment_index/total` → `n/N`; `has_refund_link` → "Entrada vinculada"; `is_fixed` →
"Conta fixa"; do contrário o método de pagamento). Entra/sai por mês e o total (entra · sai ·
dias com lançamento) somam as linhas reais — **sem agregações editoriais fabricadas** (os
rótulos agrupados do protótipo eram do mock; a tela mostra o que está lançado).

## Motion

A estrada **se desenha** (stroke-draw da linha do lançado em ~480 ms na curva do DS); o
trecho sem lastro e o traçado típico entram por fade depois dela. A grade **assenta em onda**
(stagger de ~50 ms por card). **Dinheiro nunca anima** — entra pronto. A cor de banda é
estado, não alerta: **o semáforo nunca pisca**; o único elemento que pulsa é o esqueleto de
carregamento. Com `prefers-reduced-motion`, tudo é instantâneo.

## Divergências entre o desenho e as réguas do repositório

1. **Os agrupamentos do Card 4** ("Faturas de cartão · Itaú, Amazon… → Cartões";
   "Reembolsos da casa") eram hand-authored no protótipo. A tela renderiza os **lançamentos
   projetados reais** — reconstruir os agrupamentos do mock seria paráfrase vendida como
   dado. `ui-standards`: dado antes do mapeamento do protótipo.
2. **A cor da grade é a banda do saldo de fim de mês**, não um heatmap dia-a-dia — o preview
   compacto declara a banda do mês e delega a verdade diária ao Calendário. A régua absoluta
   (`saldoHeatmap`) é a mesma da planilha.
3. **A frase da estrada não promete `min` do teto**: o menor ponto é o do caixa lançado; o
   guardrail de Hoje (`safe_to_spend_today`) é a outra régua, e a tela não o reintroduz aqui.

## Fidelidade ao método (traço às fontes)

- **A tela homônima do método** é uma grade rolante de 12 meses, dias como semáforo do saldo,
  "ampla e resumida" — radar para notar cedo o mês que aperta.
- **O buraco do futuro** é nomeado e ensinado: achar o buraco maior à frente, atravessar com
  a reserva por partes.
- **O encadeamento do Saldo até 31/12** é o mecanismo de projeção do método (a planilha real
  pré-constrói o ano inteiro de uma vez).
- **A régua de lastro** (saída lançada ≥ 60% do gasto típico) é partilhada com O ano — e vem
  do motor (`trusted_through_month`, `coverage`), fonte única.

## O que morre

- O banner de status "Seu saldo chega a R$ X" acima do gráfico — vira o veredito-herói.
- A `BalanceTrajectory` genérica no Horizonte — vira a estrada dedicada de três linhas.
- A grade de "Saldo no fim de cada mês" como cartões numéricos — vira a grade epistêmica.
- A lista de "Vencimentos próximos (60 dias)" — vira os compromissos por mês, agrupados.

## Fora de escopo

- **A grade dia-a-dia por banda real** dentro do card dos 12 meses (a verdade diária é do
  Calendário; aqui a banda é do mês).
- **Aplicar/editar cenários na própria tela** além da entrada — o side-sheet de cenários já
  existe e é reusado; o fluxo completo é dele.
- **O agrupamento editorial dos compromissos** (fundir faturas/reembolsos por rótulo) —
  avaliado depois do uso real; a tela mostra os lançamentos reais.

## Aceitação

- Os três estados do veredito renderizam com os dados que os produzem, e nenhum fabrica
  `R$ 0,00`.
- A estrada desenha lançado + zona sem lastro + traçado típico + zero + menor ponto, e o
  fold reconcilia os fins de mês em números.
- A grade mostra os quatro estados epistêmicos, colore por banda do saldo, e um dataset
  sintético apertado/negativo pinta âmbar/vermelho.
- Os compromissos agrupam por mês com `n/N` e reembolso vinculado, e os totais somam as
  linhas reais.
- `npm run check` verde, baselines visuais regeneradas do zero (desktop + mobile), React
  Doctor sem novas violações, auditoria e crítica de UI sem achados abertos.
