# Spec 029 — Onda Este mês: bento com régua do método e custo de vida

## Contexto

A tela Este mês apresentava os cálculos do mês como três tiles-herói (Performance,
Custo de vida, Economizado) seguidos de cards genéricos ("Para onde foi o dinheiro",
"Entrou × Saiu", tendência de performance). A direção da identidade define outra
leitura: a tela é **a conta que o método faz todo mês** — o que entrou, o que a vida
custou, o que sobrou guardado — organizada num **bento de duas colunas** cujo
instrumento central é a **régua de economia** (faixa-alvo 20–30% com o pino "onde
você está"), o custo de vida decomposto **por componente** num segbar, e a série
histórica do economizado como **faixa compacta de largura cheia** (evidência à
esquerda, leitura à direita).

Não há matemática nova: todos os números vêm dos DTOs existentes (`MonthMetric`,
`AnnualSavings`, `DashboardSummary`). Zero mudanças de backend.

## Estrutura da tela (ordem do DOM = ordem de leitura)

1. **Cabeçalho** — o título "Este mês" vive no shell; a tela abre com o h2 do mês
   ("Julho em números") e uma frase de contexto ("A conta que o método faz todo mês:
   o que entrou, o que a vida custou, o que sobrou guardado."). `MonthNav` ao lado no
   desktop, empilhado no mobile. O crumb da appbar mostra o mês visto ("Julho de
   2026") via store de crumbs do shell.
2. **Economia guardada** (bento, coluna 1) — o instrumento do método. Ver "Régua de
   economia" abaixo.
3. **Custo de vida** (bento, coluna 2) — total como número do corpo; segbar por
   componente (Saídas fixas · Diário · Cartão, cores `--type-*`) e a lista dos três
   com valores; estado "Dentro da renda" / "Acima da renda".
4. **Performance** (linha compacta, coluna 1) — "Sobrou dinheiro" / "Faltou dinheiro"
   com a equação que fecha com o motor (Entradas − Custo de vida − Economia −
   Patrimônio − Previsão de diário; termos zerados omitidos).
5. **Diário médio** (linha compacta, coluna 2) — média realizada por dia; estado
   "Zerado" quando 0; no mês corrente em modo cartão, o zero explica-se ("o variável
   vive no cartão").
6. **Comparado aos meses anteriores** (faixa de largura cheia) — série do
   **economizado%** dos últimos 6 meses. Ver "Série histórica" abaixo.
7. **Por titular** (quando ≥ 2 titulares) — mantido, na gramática do card da direção.

No desktop (>900px) o fluxo é grid de 2 colunas (gap uniforme `--space-6`); os cards
2–3 formam a primeira linha (alturas que conversam: o card da régua estica e ancora a
nota no pé), 4–5 a segunda, 6 e 7 em largura cheia. No mobile, coluna única na mesma
ordem do DOM — a colunização é só CSS.

**Anatomia única dos cards**: cabeçalho (ícone + título + "Como funciona?" no mesmo
endereço nos quatro) → corpo (o número grande + instrumento/contexto) → rodapé
(badge de estado, com a folga ancorada acima dele).

## Régua de economia (instrumento)

- **Número-herói**: "{pct}%" com sub "R$ {economia} de R$ {entradas} que entraram".
  O percentual exibido trunca; o julgamento de estado usa os bps brutos (o número
  nunca contradiz o veredito na fronteira).
- **Régua**: escala fixa **0→40%** com a **zona 20–30 marcada** — o mesmo instrumento
  selado para a tela O ano (assinatura proprietária; o protótipo usava 0–100, onde a
  zona vira lasca ilegível — desvio deliberado, registrado no PR). Marcas em 20% e
  30%; a régua declara o próprio recorte (marca terminal "40%"). O pino mostra a
  posição do mês; acima de 40% o pino estaciona no fim e o texto diz o valor
  verdadeiro (o número nunca mente).
- **Estados** (rótulos canônicos do método): "Nada guardado" (0%), "Abaixo do ideal"
  (<20%), "Dentro do ideal" (20–30%), "Acima do ideal" (>30%) — via `HealthBadge`.
- **Nota da régua**: o mês não é o juiz — a régua do método é a **média anual**. A
  nota sempre fecha com a leitura do ano ("No ano: {ytd}% — a régua do método é a
  média anual."), e nunca acusa um mês fraco.
- **Estado epistêmico**: com `annual_savings.economia_state === "no_record"` a régua
  não julga — sem pino, sem badge de estado; `NoRecordDash` no número e nota de
  convite ("Sem registro de economia na planilha — registre o primeiro aporte para a
  régua ler o mês.").
- **Didática atrás de pergunta**: "Como funciona?" abre o `InfoPopover` do termo
  `economizado` — nenhum parágrafo didático permanente.

## Custo de vida (decomposição)

- Total do mês no cabeçalho do card (`Money`); popover do termo `custo_de_vida`.
- **Segbar por componente** na ordem canônica das colunas (Saídas fixas · Diário ·
  Cartão), cores `--type-saida` / `--type-diario` / `--type-cartao`; segmentos
  zerados não renderizam fatia mas permanecem na lista (R$ 0,00 realizado é fato,
  não dado ausente).
- Lista dos componentes: ponto de cor + nome + valor; no mês corrente em modo cartão
  (`spending_mode === "card"`), o Diário zerado leva o contexto "Não lançado — o
  variável vive no cartão" (paridade com a Hoje).
- Estado: "Dentro da renda" (custo ≤ entradas) / "Acima da renda".
- "Para onde foi o dinheiro" morre: o segbar por componente é seu sucessor direto, e
  Economia/Patrimônio já têm morada (régua e equação da Performance).

## Série histórica (faixa compacta)

- Barras do **economizado% por mês** (últimos 6 meses até o mês visto, ordem
  cronológica), altura proporcional numa escala honesta: teto = max(40%, maior valor
  da janela) — a grade não mente a altura relativa das barras; zero é chão (barra
  mínima de 2px, visual apenas). O mês visto destaca em `--primary`; os demais em
  neutro. Valor de cada barra visível ("{pct}%"). **Cada barra é botão-atalho para o
  mês dela** (mesma navegação do `MonthNav`; nome acessível "{Mês}: {pct}% — ver o
  mês").
- **Desktop**: evidência à esquerda (barras), leitura à direita (frase). **Mobile**:
  barras em cima, frase embaixo.
- **A leitura diz o fato da série, nunca julga mês isolado**: todos zero → "O
  economizado está em zero nos últimos {n} meses — é o mesmo zero em todos, não uma
  queda."; caso geral → melhor mês e posição do mês visto na janela, com a régua
  anual como fecho.
- A tendência de **performance** morre nesta tela: a leitura mês-a-mês da performance
  pertence à tela O ano; aqui a série serve ao instrumento da tela (economia).

## Primitivas novas do DS

- **`RangeRuler`** (`src/design-system/components/`): trilho + zona-alvo + marcas +
  pino. Decorativa por padrão; com `label`, `role="img"` e texto equivalente
  completo. A onda O ano reutiliza (escala 0→40, zona 20–30).
- **`SegBar`**: barra de composição multissegmento (fatias por fração + cor), mesma
  política de acessibilidade. Sucessora do markup artesanal de segbar.
- O `StatusChip` local da tela morre em favor do **`HealthBadge`** do DS.

## O que morre

- Os três tiles-herói: seus números renascem nos cards do bento (a tela não perde
  nenhum dado — Performance, Custo de vida, Economizado, Diário médio e a
  decomposição seguem visíveis).
- "Para onde foi o dinheiro" (sucedido pelo segbar por componente) e "Entrou × Saiu"
  (a conta vive na equação da Performance).
- A tendência de performance (a série histórica da tela é do economizado; a leitura
  anual da performance pertence a O ano).
- O `StatusChip` reimplementado localmente.

## Motion

Coreografia de entrada única por montagem (fade+translate curto com stagger, CSS puro no
orçamento de entrada do app — ~400ms do início ao fim, governado pelos tokens `--dur-*` e
nunca dentro de media query, que o toggle "Animações" não alcança). Superfícies transitam; **dinheiro e
percentuais nunca animam** (sem count-up; o pino da régua não desliza ao montar).

## Acessibilidade

- Hierarquia: título no shell; h2 do mês; h3 por card. Cards como `section` com
  `aria-label`/`aria-labelledby`.
- Régua e segbar com texto equivalente completo (`role="img"`) — o dado nunca vive
  só na cor ou na geometria.
- Texto e `aria` dizem o valor verdadeiro (truncamento só de exibição; barra satura,
  número não).
- Contraste AA nos 2 temas × 6 acentos; alvos ≥ 44px; ordem do DOM = ordem de
  leitura.

## Fora de escopo

- Mudanças de backend (todos os dados já existem nos DTOs).
- A tela O ano (onda própria — reutiliza `RangeRuler`).
- Estados epistêmicos novos por mês (o `economia_state` anual cobre o caso real).

## Aceitação

1. Bento de 2 colunas no desktop com régua 0→40 (zona 20–30, pino do mês, estados
   canônicos) e custo de vida com segbar por componente; coluna única no mobile na
   mesma ordem do DOM.
2. Série histórica do economizado como faixa compacta de largura cheia (evidência à
   esquerda, leitura à direita no desktop), escala honesta, mês visto destacado.
3. Performance e Diário médio seguem na tela com rótulos canônicos ("Sobrou/Faltou
   dinheiro", "Zerado") e a equação fecha com o motor.
4. `RangeRuler` e `SegBar` no DS com testes; `HealthBadge` substitui o chip local;
   helpers de status puros e testados (novo estado "Nada guardado").
5. Gates: `npm run check` verde; react-doctor sem achados novos; e2e visual com
   baselines regenerados do zero (2×); impeccable audit + critique; copy
   sentence-case; dinheiro tabular e estático.
