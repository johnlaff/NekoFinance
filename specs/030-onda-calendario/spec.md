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

1. **Veredito** — a abertura cabe em duas linhas (regras 41 e 42): um olho que
   declara a costura ("Realizado até 10/06") com o "Como funciona?" e a navegação
   de mês na mesma linha, e a manchete com a forma do mês ("Junho afunda no dia 20
   e respira no 25."). Sem corpo: todo número que ele diria já está impresso
   abaixo. O rótulo do mês fica em `sr-only` no `MonthNav` (`hideLabel`) — o crumb
   da appbar já o mostra, e o texto no DOM é o que a `aria-live` anuncia.
2. **Trilho** — o saldo do mês numa linha do tamanho de uma frase, sólida no
   realizado e tracejada na projeção, com marcadores em hoje, no menor saldo e nas
   entradas. **Só aparece no celular**: no desktop a célula tem altura para
   movimento e saldo, e a forma do mês se lê na própria grade. O DOM é único (regra 10) — quem esconde é o CSS, e o SVG é `aria-hidden` nos dois viewports.
3. **Grade do mês** — 7 colunas, semana começa na segunda (Seg → Dom, como a
   direção). Anatomia por viewport abaixo.
4. **O dia aberto** — painel de ~340px à direita no desktop, bloco abaixo da grade
   no celular. O saldo é o herói e a faixa do termômetro vem em palavra ao lado
   dele. O dia nasce em **hoje** no mês corrente, no **dia 1** nos demais.
5. **O que marca o mês** — menor saldo, maior saída e entradas, cada linha
   navegando para o dia. Quando o vale e a maior saída caem no mesmo dia, a data
   aparece uma vez e a linha nomeia os dois papéis.

Não há legenda de cores: ela era didática fixa (regra 1) e a explicação inteira
mora no "Como funciona?", faixas do termômetro incluídas.

## Grade — anatomia por viewport

A célula é um `gridcell` real (padrão APG de grade de datas): a grade usa
`role="grid"` com linhas semanais e **roving tabindex** — setas movem ±1 (Esq/Dir) e
±7 (Cima/Baixo), Home/End vão ao início/fim da semana, PageUp/PageDown trocam o mês,
Enter/Espaço selecionam o dia (a seleção também acontece no próprio foco de clique).
Tab entra e sai da grade uma única vez.

As linhas da grade dividem a altura disponível no desktop (`grid-auto-rows:
minmax(76px, 1fr)` sobre `--content-h`, publicada pelo shell): o dado ocupa a tela
que é dele, em vez de parar no meio da viewport.

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

**Mobile (≤900px)** — a célula tem 46×52px e carrega **dia e saldo** (reais
inteiros com milhar, o mesmo formato do desktop). O movimento é o que não cabe: ele
mora no dia aberto.

- Superfície neutra. **O termômetro pinta só as faixas que apertam** (Apertado,
  Negativo, Crítico): os limiares seguem absolutos em reais, o que muda é onde a
  cor é gasta — num mês inteiro na faixa boa, 30 células tingidas não distinguem
  nada. Sobre o tint, o número do dia sobe para `--text-strong` e o saldo crítico
  usa `--danger-300`, ambos para manter AA.
- Eventos: anel de acento em hoje, preenchimento no dia aberto (dois conceitos,
  duas formas), triângulo verde na entrada (forma além de cor) e contorno âmbar no
  menor saldo; previsto recua a superfície e esmaece o número — nunca `opacity`
  global, que apagaria também o tint de aperto.
- Alvo de toque ≥ 44px (gap reduz em <380px antes de encolher a célula).

A frase didática do "Como funciona?" cobre as duas leituras (eventos no desktop,
termômetro no mobile) — o copy inline não bifurca por viewport (regra 8). O que
varia entre viewports é instrumento, não texto: o trilho existe só no celular, e a
grade do celular troca movimento por saldo porque 46px não comportam os dois.

## Movimento líquido (a conta)

O movimento do dia é o **delta da corrente do saldo**: `saldo(d) − saldo(d−1)`.
Derivar dos componentes (`income − fixed − daily`) mentiria nos dias com economia
lançada (o `MonthGridDay` não carrega `economia_cents`); o delta da corrente captura
qualquer movimento por construção. Para o dia 1, a véspera vem do grid do mês
anterior (uma chamada `get_month_grid` extra; dezembro→janeiro cruza o ano). Sem
véspera conhecida, o movimento do dia 1 não renderiza (dado ausente ≠ zero).

Saldo por dia: passado vem do `get_month_grid` (planilha restaurada), hoje-em-diante
do `get_forecast` (projeção) — mesma costura da tela atual.

## O dia aberto (anatomia única, dois endereços)

- **Título**: dia por extenso ("Sábado, 12 de julho") + sufixo "· previsto" quando o
  dia é futuro (mesma gramática da grade).
- **Saldo herói**: o saldo que o dia deixou em `--fs-money-lg`, com a faixa do
  termômetro em palavra ao lado ("Folga", "Apertado") — cor nunca é o único canal.
- **Movimento do dia**: o delta contra a véspera, logo abaixo do saldo.
- **Lançamentos do dia**: linhas de `getMonthTransactions` filtradas pela data —
  descrição + valor (`Money`), pílulas herdadas do Livro-razão só quando carregam
  estado (parcela, Previsto, Reembolso). Dia sem lançamentos: "Sem movimento — o
  saldo ficou como estava." (o vazio é fato, não erro).
- **Resumo do dia**: componentes não-zerados (Entrada · Saídas fixas · Diário ·
  Economia) — zerados são omitidos para a leitura não virar formulário.
- **Sem corrente para o dia**: travessão epistêmico no lugar do saldo herói —
  nunca `R$ 0,00` fabricado.
- **Rodapé**: link "Ver no Livro-razão", ancorado no fim do painel (regra 14) —
  navega para Lançamentos (o mês visto).

## O que morre — e por quê

- **Aba "Ano inteiro"**: o instrumento anual do método é a tela O ano (tribunal) e o
  radar de 12 meses é o Horizonte — o heatmap anual aqui era um terceiro endereço
  para a mesma pergunta, fora da direção. O `SegmentedControl` sai com ela.
- **Célula → Lançamentos**: o clique passa a selecionar o dia na agenda (o link para
  o Livro-razão vive no rodapé da agenda). A tela deixa de ser um atalho cego.
- **Heatmap contínuo do termômetro**: a saúde vira dado da célula (saldo visível) e
  o evento "menor saldo" guarda o pior ponto. No celular o tint sobrevive só nas
  faixas que apertam — pintar as 30 células de um mês saudável não distingue nada.
- **Legenda de cores**: era didática fixa ocupando espaço permanente (regra 1) e
  nenhum calendário de mercado a mantém. A explicação inteira foi para o "Como
  funciona?".

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
  empate, entrada), agrupamento da agenda, **manchete do mês** (vale × respiração,
  ordem cronológica), **marcos do mês** (colisão vale/maior-saída), **faixa da
  grade** (só o que aperta) e **série do trilho** (normalização, corte
  realizado × projeção) — TDD.
- Testes de tela: estados (loading/erro/dados/mês sem corrente), seleção default,
  roving tabindex, aba anual ausente, e a ausência da legenda fixa.
- E2E visual: baselines regenerados do zero (rm -rf + 2 execuções), shots mobile e
  desktop com a agenda visível; React Doctor sem achados novos; impeccable audit +
  critique antes do PR.
