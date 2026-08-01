# Spec 027 — Onda Hoje: veredito-herói, modo cartão e curadoria da Mia

## Contexto

A fundação "Midnight Purr" (spec 024) trocou tokens e shell, mas a tela Hoje mantém o
layout anterior: herói denso com `dl` de stats, mini-gráfico de trajetória e grid de dois
cards (check-in com registro inline + contas a vencer). A direção "Conversa com a Mia"
define outra composição para a tela de uso diário: a saudação-veredito como herói, a
curadoria explícita da assistente, o bloco do dia com as faturas em aberto como corpo (o
velocímetro de quem vive no crédito), os próximos movimentos e o par saldo+reserva com
réguas — com o insight na voz da Mia como divisor tingido, nunca caixa dentro de caixa.

Esta spec aterrissa a Onda Hoje sobre o substrato que as specs 025 (estados epistêmicos +
modo de gasto) e 026 (domínio do cartão) deixaram pronto. Não há matemática nova de
domínio: a tela apresenta números que o motor já deriva; as duas únicas adições de DTO
são leituras SQL diretas.

## Estrutura da tela (ordem do fluxo)

1. **Saudação-veredito (herói, `[data-large-title]`)** — gato (NekoMark grande), h1 de
   saudação por hora local ("Bom dia." · "Boa tarde." · "Boa noite." — sem nome: o app
   não tem fonte de nome de usuário e veredito nunca nasce de dado fabricado), veredito
   "Pode gastar hoje {valor}" com o valor em cor de acento, segunda linha em destaque com
   o guardrail que morde, e a camada didática (parágrafo `teach`) explicando o limite e o
   estado do teto. Primeiro adotante da coordenação large-title: no mobile o título da
   appbar só assume quando o herói sai de vista.
2. **Linha de curadoria** — avatar da Mia + "A Mia separou o que importa hoje — a ordem
   muda com o seu dia, os números nunca." (contrato de honestidade: curadoria é ordem,
   nunca número).
3. **Bloco do dia** (card, coluna esquerda no desktop) — ver "Bloco do dia" abaixo.
4. **Insight do mês** (voz da Mia) — fechamento previsto + ponto mais apertado, ver
   "Insight" abaixo.
5. **Próximos movimentos** (card) — contas a vencer (45 dias) + próxima entrada prevista
   - par custo de vida do mês / Guardado%.
6. **Saldo e reserva** (card) — par de stats com réguas (termômetro do saldo · meses de
   reserva) + insight de reserva por estado epistêmico.

Estados de tela preservados do comportamento atual: erro sem cache (EmptyState com
retry — nunca R$ 0,00 fabricado), skeleton no primeiro load, banner "dados antigos"
quando há cache, aviso de preview web.

### Desktop (>900px)

Grid de 2 colunas (`minmax(0,1.08fr) minmax(0,.92fr)`, mesma razão do protótipo da
direção): herói e curadoria atravessam as duas colunas; o bloco do dia ancora a coluna
esquerda (`grid-row: span 3`); insight, próximos movimentos e saldo+reserva compõem a
direita. Tablet (701–900px) e mobile empilham.

### Mobile (≤700px)

Empilha na ordem do fluxo. O herói marca `[data-large-title]`; a appbar mostra a data
("Quarta, 15 de julho") como crumb da Hoje — o shell ganha a prop `crumbs` (override
por tela do crumb de `SCREEN_META`), preenchida pelo App com a data local formatada.
Próximos movimentos viram carrossel horizontal com scroll-snap (cards de polegar); no
desktop a mesma lista rende como linhas (a linguagem da densidade de mouse).

## Veredito e camada didática (matriz de copy)

Valor: `max(0, forecast.safe_to_spend_today_cents)`, `Money` display — tabular, nunca
anima. Segunda linha (negrito) pelo `binding_guardrail`:

- `cash` → "Sem deixar nenhum dia no vermelho."
- `savings` → "Sem tocar na economia planejada do ano."

Parágrafo didático (`teach`), composto por duas partes:

**Parte 1 — o que é o número** (por modo):

- Modo cartão: "Este é o limite do caixa: o maior gasto por dia que o saldo aguenta até
  {último dia do mês}. No cartão, a compra pesa na fatura seguinte — este número protege
  o caixa deste mês."
- Modo débito: "É o menor de dois limites: o teto diário de {X} e o que o caixa aguenta
  sem nenhum dia no vermelho até {último dia do mês}." (sem teto: só a perna do caixa).

**Parte 2 — o estado do teto** (por `daily_ceiling_source`):

- `chosen`: "O teto que você estipulou — {X} por dia — segue como referência." (link
  discreto para a tela do teto).
- `estimate`: mesmo texto com selo `EstimateMark` junto ao valor.
- `none`: "O método pede um segundo limite, o teto diário que você estipula — e ele ainda
  não está definido." + CTA "Estipular o teto".
- `ceiling_proposal_pending`: substitui o CTA por "A planilha propõe um teto — revisar."
  (overlay de proposta, nunca o número).

## Bloco do dia

Cabeçalho: "Gasto variável de hoje" + ação "Ver tudo ›" (→ Lançamentos). Total do dia +
frase de estado + `ModeChip` (com gate — sai do herói e vive aqui, como no protótipo).

### Modo cartão (corpo = faturas em aberto por vencimento)

- Total = `card_spend_today_cents` (novo no `DashboardSummary`): magnitude das compras
  de cartão realizadas hoje. Frase: zero → "— nada somado à fatura hoje"; senão
  "— somado às faturas de hoje".
- **Resumo das faturas**: "Faturas em aberto — {n} {cartão|cartões}" + total (Σ das
  `upcoming_invoices` com status `aberta`/`fechada`) + barra de proporção contra o gasto
  típico de um mês (`forecast.baseline_outflow_cents` — denominador estável; o custo de
  vida do mês corrente ainda está se formando) + nota didática: "É aqui que o seu gasto
  variável mora: cada compra soma na fatura do cartão usado. O método manda deixá-las à
  vista — a fatura crescendo é o velocímetro de quem gasta no crédito."
- **Agrupamento por vencimento**: um cabeçalho "Vence(m) em {data}" por `due_date`, em
  ordem cronológica; dentro do grupo, linhas por cartão (maior primeiro). Linha: nome do
  cartão + contexto pequeno + valor. Contextos: "A maior fatura em aberto" (maior de
  todas), "Fechada — aguarda pagamento" (status `fechada`), "Acumulando" (demais),
  dono quando não é o padrão ("De {dono}"). Etiqueta "Reembolso" (verde de status, tint)
  quando `has_refund_expectation` (novo no `UpcomingInvoiceDto`).
- **Rodapé de zerados**: cartões cadastrados (via `list_cards`) sem fatura
  aberta/fechada — "{nomes} sem fatura em aberto — cartão parado sai da lista sozinho e
  volta quando você usar." Cartão nunca some em silêncio.
- **Glossário do modo** (didática fixa): o Diário zerado é legítimo por design; débito e
  Pix mexem o saldo na hora; migrar para o débito devolve o check-in ao diário.

### Modo débito (corpo = check-in do teto)

Total = `daily_spend_today` + "/ teto {X}" com os estados do teto (selo de estimativa;
sem teto → travessão + CTA). Barra de progresso única (acima do teto → cor de perigo).
A decomposição por lançamento (peças na barra) fica para a onda de Lançamentos — exigiria
a lista de transações do dia, que este DTO não carrega.

### O que morre no bloco

O registro inline (radiogroup de tipos + input + "Registrar") sai da tela: o bloco do dia
é superfície de leitura; registrar é ação de primeira classe do shell (FAB no dock,
CTA da sidebar, tecla N → Compose com aprovação explícita — contrato da direção). O
estado "Em dia. Você já lançou hoje." é absorvido pela frase do total.

## Insight do mês (voz da Mia)

Divisor tingido com o acento (tint de fundo + borda, mesma geometria dos cards — voz,
não caixa), avatar da Mia como faísca. Conteúdo derivado de `forecast.daily` do mês
corrente (helper puro, TDD):

- Fechamento: "Fechando o dia assim, {mês} termina em {rótulo do saldoBand} — saldo
  previsto de {valor}."
- Ponto mais apertado: "O ponto mais apertado do mês é {hoje|dia N}: {valor}" + ", e a
  próxima entrada chega dia {N}." quando houver entrada futura no mês.
- Sem déficit: "Nenhum dia no vermelho à vista — no método, isso é ficar sem 'buraco do
  futuro'." Com déficit: "{N} dia(s) ficam no vermelho — o buraco do futuro do método.
  O menor ponto: {valor}, dia {N}." (valor em cor de dinheiro negativo; tom de guia,
  nunca punição).

O mini-gráfico de trajetória morre nesta tela: o insight carrega a leitura em linguagem
natural e o instrumento gráfico do caixa é a tela Horizonte (decisão da rodada de
identidade — "o radar do caixa é tela própria").

## Próximos movimentos

- Linhas/cards: `get_upcoming_bills(45)` como hoje (data + descrição + valor), com
  projeção marcada. Sem tile agregado de faturas: os vencimentos pré-lançados da
  planilha já chegam como contas — agregar as faturas de novo mostraria o mesmo
  compromisso duas vezes, e o velocímetro delas é o bloco do dia.
- **Próxima entrada**: primeira `forecast.daily` futura com `income_cents > 0` —
  "Entrada prevista" + valor positivo (verde de status) + data. (Descrição real não
  existe no DTO; rótulo honesto.)
- Rodapé do card: par "Custo de vida no mês" (`currentMonthMetric.cost_of_living_cents`)
  × "Guardado" (`savings_rate_bps` do mês; `economia_state = no_record` → sobra derivada
  como estimativa marcada, regra do D4 da spec 025).

## Saldo e reserva

Par de stats com réguas (mesma gramática do protótipo):

- **Saldo hoje**: valor + gauge horizontal com a cor/rótulo do `saldoBand` (termômetro
  absoluto e canônico da planilha) + frase "Termômetro {rótulo} — {referência da faixa}".
- **Reserva**: por `reserve_state` — `verdict`: "{X} meses" + gauge (alvo 6 meses);
  `estimate`: idem + selo (retrato vivo); `zero`: palavra dedicada "Sem reserva";
  `no_record`: travessão + CTA "Mapear" (→ Configurações).
- **Insight de reserva** (dentro do card, divisor com voz): copy por estado — sem
  registro: "A planilha não informa uma reserva guardada — e saldo em conta não é
  reserva. O método pede o equivalente a 6 meses do custo de vida num lugar separado;
  quando você registrar, o Neko acompanha aqui." Zero e below análogos, sempre guiando.

## Backend (duas adições de leitura, TDD)

1. `DashboardSummary.card_spend_today_cents` — `SUM(ABS(amount))` das despesas de hoje
   com `payment_method='credit'` ou `invoice_id` presente, `is_projection=0`, fora de
   cenário. Espelho do `daily_spend` existente para o outro modo.
2. `UpcomingInvoiceDto.has_refund_expectation` — `EXISTS` de Entrada com
   `refund_invoice_id` da fatura. Habilita a etiqueta "Reembolso" sem N+1 de
   `get_invoice`.

## Motion

Coreografia de entrada única por montagem: herói → curadoria → bloco do dia → insight →
demais cards, fade+translate curto no orçamento de entrada do app (~400ms do início ao fim
da sequência, escalonamento em `--dur-stagger-step`), CSS puro governado pelos tokens
`--dur-*` — nunca dentro de media query, que o toggle "Animações" não alcança. Superfícies transitam; **dinheiro nunca anima**
(o valor viaja com a superfície, nunca sozinho — contrato do DS). Nenhuma animação de
número, count-up ou morph.

## Acessibilidade

- Hierarquia: h1 no veredito, h2 nos títulos de card; faturas como listas (`ul/li`).
- Gauges com texto equivalente (a frase da faixa é o dado; a barra é reforço visual).
- Alvos ≥44px no mobile; contraste AA nos dois temas × 6 acentos (tokens já auditados
  na fundação; verificação pontual nos tints novos).
- Carrossel mobile com scroll nativo; sem sequestro de foco.

## Fora de escopo

- Composer/porta de conversa na Hoje (tela da Mia, onda própria).
- Peças por lançamento na barra do modo débito (onda de Lançamentos).
- Redesenho do Compose/registro (contrato do #176 já implementado no shell).
- Write-back, séries e drill de fatura (tela Cartões).

## Aceitação

1. A tela renderiza a composição nova nos três ranges (desktop 2 colunas · tablet ·
   mobile empilhado) com `[data-large-title]` coordenando a appbar no mobile.
2. Modo cartão: faturas em aberto agrupadas por vencimento com total, contextos,
   etiqueta de reembolso e rodapé de zerados; modo débito: check-in do teto com estados
   epistêmicos. Nunca `R$ 0,00` fabricado; travessão/selo/palavra dedicada nos estados.
3. Veredito coerente com `binding_guardrail` e com o estado do teto (matriz de copy).
4. Insight derivado por helper puro testado; sem matemática nova fora do motor.
5. Gates: `npm run check` verde; react-doctor zerado; e2e visual com baselines
   regenerados do zero (relógio congelado nos testes — saudação e data são funções da
   hora); impeccable audit + critique; copy sentence-case; dinheiro tabular e estático.
