# Spec 030 — Onda Calendário: grade de saldo com agenda master-detail

## Contexto

A tela Calendário apresenta o mês como heatmap do termômetro do saldo (célula = dia +
tint da faixa + saldo compacto), com uma aba "Ano inteiro" que replica o heatmap em
12 minigrades. A direção da identidade redefine a tela: **cada dia carrega o movimento
e o saldo que ele deixou**; a gramática realizado × previsto vem do **estilo de
borda** (sólida × tracejada); as **cores marcam os eventos do mês** — hoje, entradas
e o menor saldo; o dia tocado abre a **agenda** — num painel fixo à direita no
desktop (master-detail), abaixo da grade no mobile.

Não há matemática nova: todos os números vêm dos DTOs existentes (`MonthGridDay`,
`ForecastDay`, `TransactionRow`). Zero mudanças de backend.

## Estrutura da tela (ordem do DOM = ordem de leitura)

1. **Cabeçalho** — o título "Calendário" vive no shell; a tela abre com o h2 do mês
   ("Julho dia a dia") e uma frase de contexto ("Cada dia mostra o movimento e o
   saldo que ele deixou.") com o "Como funciona?" ao lado (didática atrás de
   pergunta). `MonthNav` ao lado no desktop, empilhado no mobile. O crumb da appbar
   mostra o mês visto ("Julho de 2026") via store de crumbs do shell.
2. **Grade do mês** — 7 colunas, semana começa na segunda (Seg → Dom, como a
   direção). Anatomia por viewport abaixo.
3. **Legenda** — imediatamente abaixo da grade, declara as cores em uso no viewport.
4. **Agenda do dia** — painel fixo (sticky) de ~340px à direita no desktop,
   ocupando a segunda coluna do grid da tela; no mobile, logo abaixo da legenda.
   O dia selecionado nasce em **hoje** no mês corrente, no **dia 1** nos demais.

## Grade — anatomia por viewport

A célula é um `gridcell` real (padrão APG de grade de datas): a grade usa
`role="grid"` com linhas semanais e **roving tabindex** — setas movem ±1 (Esq/Dir) e
±7 (Cima/Baixo), Home/End vão ao início/fim da semana, PageUp/PageDown trocam o mês,
Enter/Espaço selecionam o dia (a seleção também acontece no próprio foco de clique).
Tab entra e sai da grade uma única vez.

**Desktop (>900px)** — a célula fala:

- Conteúdo: número do dia + **movimento líquido** do dia (compacto, com sinal; cor
  `--money-pos` quando positivo; omitido quando zero) + **saldo** que o dia deixou
  (compacto, `Money`-tabular).
- Superfície neutra (`--surface`); a cor é gasta só nos eventos:
  - **Hoje**: borda + número do dia no acento da marca, tint sutil de acento.
  - **Entrada** (dia com `income_cents > 0`): borda na cor positiva (mistura ~40%).
  - **Menor saldo do mês**: borda `--warning` e saldo em `--warning` — o dia que
    prova o "pode gastar" (primeiro dia em caso de empate).
  - **Previsto** (dia > hoje): borda tracejada, sem superfície, texto esmaecido —
    realizado × previsto é gramática de borda, nunca de cor.
- Dia selecionado: `outline` de 2px no acento (não muda a borda de evento).
- Dias fora do mês: célula vazia, sem borda, fora da navegação do teclado.

**Mobile (≤900px)** — a grade é navegação (fallback deliberado, não planilha
encolhida): em 390px, 7 colunas × 3 dados dá ~50px por célula — números de 5
dígitos virariam fonte de 8px.

- Célula quadrada (aspect-ratio 1) só com o **número do dia** sobre o **tint do
  termômetro** (faixas fixas em reais — `saldoBand`, a "saúde" do dia). Movimento e
  saldo não renderizam: os números moram na agenda do dia tocado.
- Eventos viram sinais discretos: ponto sob o número (entrada = positivo, menor
  saldo = warning), anel de acento para hoje; previsto esmaece o tint (opacity).
- Alvo de toque ≥ 44px (gap reduz em <380px antes de encolher a célula).

A frase didática do "Como funciona?" cobre as duas leituras (eventos no desktop,
termômetro no mobile) — o copy inline não bifurca por viewport (regra 6); só a
**legenda** muda, porque descreve as cores que o viewport realmente usa:

- Desktop: Hoje · Entrada · Menor saldo do mês · Previsto — ainda não aconteceu.
- Mobile: as 5 faixas do termômetro (Folga · OK · Apertado · Negativo · Crítico) +
  Hoje · Entrada · Menor saldo · Previsto.

## Movimento líquido (a conta)

O movimento do dia é o **delta da corrente do saldo**: `saldo(d) − saldo(d−1)`.
Derivar dos componentes (`income − fixed − daily`) mentiria nos dias com economia
lançada (o `MonthGridDay` não carrega `economia_cents`); o delta da corrente captura
qualquer movimento por construção. Para o dia 1, a véspera vem do grid do mês
anterior (uma chamada `get_month_grid` extra; dezembro→janeiro cruza o ano). Sem
véspera conhecida, o movimento do dia 1 não renderiza (dado ausente ≠ zero).

Saldo por dia: passado vem do `get_month_grid` (planilha restaurada), hoje-em-diante
do `get_forecast` (projeção) — mesma costura da tela atual.

## Agenda do dia (anatomia única, dois endereços)

- **Título**: dia por extenso ("Sábado, 12 de julho") + tag "Previsto — ainda não
  aconteceu" quando o dia é futuro (mesma gramática da grade).
- **Lançamentos do dia**: linhas de `getMonthTransactions` filtradas pela data —
  descrição + valor (`Money`), pílulas herdadas do Livro-razão só quando carregam
  estado (parcela, Previsto, Reembolso). Dia sem lançamentos: "Sem movimento — o
  saldo ficou como estava." (o vazio é fato, não erro).
- **Resumo do dia**: componentes não-zerados (Entrada · Saídas fixas · Diário ·
  Economia) — zerados são omitidos para a leitura não virar formulário.
- **Saldo que o dia deixou**: a última linha, sempre presente (ou travessão
  epistêmico quando não há corrente para o dia).
- **Rodapé**: link "Ver no Livro-razão" — navega para Lançamentos (o mês da agenda).

## O que morre — e por quê

- **Aba "Ano inteiro"**: o instrumento anual do método é a tela O ano (tribunal) e o
  radar de 12 meses é o Horizonte — o heatmap anual aqui era um terceiro endereço
  para a mesma pergunta, fora da direção. O `SegmentedControl` sai com ela.
- **Célula → Lançamentos**: o clique passa a selecionar o dia na agenda (o link para
  o Livro-razão vive no rodapé da agenda). A tela deixa de ser um atalho cego.
- **Heatmap do termômetro no desktop**: a saúde vira dado da célula (saldo visível)
  e o evento "menor saldo" guarda o pior ponto; o tint contínuo permanece no mobile,
  onde a célula não tem espaço para o número.

## Estados

- **Carregando**: `EmptyState` skeleton (`role="status"`).
- **Erro / sem dados**: `EmptyState` com convite único ("Importe a planilha para ver
  o saldo dia a dia.").
- **Mês sem corrente** (navegou para além dos dados): células com travessão, agenda
  com o vazio honesto — nunca `R$ 0,00` fabricado.
- **Preview web** (`!isTauri`): aviso discreto mantido.

## Acessibilidade

- Grade: `role="grid"` + `aria-label` com o mês; linhas `role="row"`; células
  `role="gridcell"` com `aria-selected` e rótulo completo ("12 de julho · Saldo
  R$ 5.569,65 · Menor saldo do mês"); cabeçalhos de dia da semana `columnheader`.
- Roving tabindex conforme APG; foco visível (`outline` de acento).
- Agenda: `aria-live="polite"` para a troca de dia anunciar; título é heading.
- Cores nunca são o único canal: eventos têm texto no rótulo da célula e na agenda;
  contraste AA nos dois temas.

## Verificação

- View-model puro (`calendarioView.ts`): matriz Seg-first, costura das correntes,
  delta do movimento (fronteira de mês/ano), detecção de eventos (menor saldo com
  empate, entrada), agrupamento da agenda — TDD.
- Testes de tela: estados (loading/erro/dados), seleção default, roving tabindex,
  legenda por viewport, aba anual ausente.
- E2E visual: baselines regenerados do zero (rm -rf + 2 execuções), shots mobile e
  desktop com a agenda visível; React Doctor sem achados novos; impeccable audit +
  critique antes do PR.
