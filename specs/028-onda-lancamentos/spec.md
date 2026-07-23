# Spec 028 — Onda Lançamentos: célula×nota com daymarks e colunas de desktop

## Contexto

O Livro-razão apresentava lançamentos como linhas expansíveis (data, tipo, descrição e
valor), com os itens da nota escondidos atrás de um toque e a divergência item×célula
reportada como aviso interno ("Itens não batem"). A direção da identidade define outra
leitura: a **célula é a autoridade** — cada dia expõe, por coluna do método, o total da
célula; a nota itemiza; e a diferença célula×nota aparece como **linha sintética de
reconciliação**, nunca como item. A lista explode as notas em linhas de primeira classe,
agrupadas por dia (daymarks), com colunas de verdade no desktop e metadados como pílulas
junto do nome — nunca na coluna do dinheiro.

Não há matemática nova de domínio: o import já grava o total da célula como `amount` do
lançamento e as linhas da nota como `line_items` (a divergência é derivável por
`|amount| − Σ|itens|`). A única adição de backend é uma leitura SQL (vínculo de
reembolso por linha).

## Estrutura da tela (ordem do fluxo)

1. **Cabeçalho** — o título "Lançamentos" vive no shell (sh-top no desktop, appbar no
   mobile — nenhuma tela o duplica); a tela abre com a frase de contexto ("Tudo o que
   entrou e saiu, do mais recente para o mais antigo."). No desktop, a busca mora ao
   lado dela (o par clássico título-à-esquerda / busca-à-direita); no mobile ela desce
   para o rodapé da lista (zona do polegar). O crumb da appbar mostra o mês visto
   ("Julho de 2026") via store de crumbs do shell.
2. **Linha de filtros** — `MonthNav` (mês visto; "Hoje" volta ao corrente) + filtro por
   tipo: **chips inline no desktop** (Todos + os 5 tipos na ordem canônica de
   `FORM_KINDS`), **bottom sheet no mobile** (gatilho "Tipo: {atual} ▾"; dialog nativo
   com opções e didática curta dos tipos). À direita, quando o motor cobre o mês visto:
   "Custo de vida — {X} no mês" (nunca fabricado; oculto sem dado).
3. **Obrigações recorrentes** (`ObligationsCard`) — só quando há obrigações (ou erro de
   carga): o estado vazio não renderiza card algum. A primeira dobra pertence ao
   conteúdo primário, e a descoberta da feature vive na ação de marcar item.
4. **Lista por daymarks** — ver "Modelo célula×nota" abaixo.
5. **Busca no rodapé (mobile)** — input de busca in-flow após a lista, com clearance
   para o dock do shell.

### Desktop (>900px)

Coluna única de leitura (`max-width` ~880px). Linhas em grade de colunas de verdade:
`ícone | nome | contexto | valor` (`grid-template-columns: 34px minmax(0,1fr)
minmax(0,1.15fr) 110px`) — o contexto deixa de correr atrás do nome e vira coluna
própria; é o alinhamento vertical que faz a lista escanear como desktop. O daymark
alinha à esquerda com o fio completando a linha (separador centrado é idioma de feed de
celular). A linha de reconciliação corre como frase única, fora da grade.

### Mobile (≤700px)

Linhas em flex (ícone 42px + nome/contexto empilhados + valor à direita). Daymark
centrado entre fios. Filtro em bottom sheet; busca no rodapé da lista. Alvos ≥44px.

## Modelo célula×nota (o corpo da tela)

### Agrupamento

- **Daymark por dia**: "{Dia da semana}, {d} de {mês}" (+ chip "Hoje" no dia corrente;
  chip de Saldo encadeado do fim do dia à direita, com a cor do termômetro absoluto —
  paridade com a coluna Saldo da planilha, preservada da tela atual).
- **Dentro do dia, um grupo por tipo de movimento** (a célula é (dia, coluna)):
  cabeçalho de célula (`cel-head`) com o rótulo à esquerda e o total à direita.
  - Tipos com célula na planilha (Entrada, Saída, Diário): "{Tipo} — Total da célula".
  - Cartão (compra pendura na fatura, não na célula do dia): "Cartão — soma na fatura".
  - Economia (aba própria): "Economia — total do dia".
  - O total do grupo é Σ|amount| dos lançamentos do grupo — para célula importada,
    exatamente o valor da célula (o import preserva a autoridade).
- **Ordem dos dias — distância de hoje**: passado em ordem decrescente (o mais recente
  primeiro); dias futuros do mês corrente ficam atrás de um `Disclosure` ("O que ainda
  vem neste mês — {n} lançamentos · {Σ}") no topo, em ordem crescente (o próximo
  primeiro); mês inteiramente futuro lista em ordem crescente sem disclosure. A primeira
  dobra do mobile pertence ao realizado.

### Linhas

- Lançamento **itemizado** (`line_items.length > 0`): cada item vira uma linha —
  ícone circular na cor do kind do item, nome (descrição do item), contexto
  ("{Kind}" + " · {Seção}" quando a nota tem seção), valor.
- Lançamento **simples**: a própria linha — ícone do tipo, descrição, contexto
  ("{Tipo}" + qualificadores: "vence {data}" quando há `due_date`).
- **Pílulas junto do nome, nunca na coluna do dinheiro**: parcela `n/N` (mono),
  "Previsto" (`is_projection`/futuro), "Reembolso" (verde de status, tint — dinheiro
  que volta), tags do lançamento. Em célula itemizada, as tags do lançamento-pai
  aparecem no `cel-head` (ali o cabeçalho representa o lançamento importado).
- **Interações preservadas**: toque expande o painel de ações (Editar · Tags · Apagar,
  com escopo de série; importado desabilita edição/exclusão com nota); em linha de item,
  o painel é do lançamento-pai e inclui `MarkObligationAction` do item. Sem drill novo.

### Linha sintética de reconciliação

Quando `||amount| − Σ|itens|| > 1` centavo num lançamento itemizado:

- O `cel-head` ganha o selo **"Com diferença"** (cor de atenção, tint).
- O grupo fecha com a linha sintética: "**Diferença no detalhamento** — O total da
  célula é {maior|menor} que a soma dos itens da nota." + |diferença| — sem ícone, sem
  toque (`aria-disabled`), separada por fio tracejado, cor de atenção. Nunca conta como
  item nem soma no grupo.
- Com **busca ativa**, a linha e o selo se escondem (itens visíveis são subconjunto —
  comparar contra o total da célula mentiria); o filtro por tipo não esconde (seleciona
  células inteiras).

## Busca e filtro

- Busca normalizada (case/acento-insensível) sobre descrição do lançamento, descrições
  dos itens e data; aplica por linha, esconde grupos e daymarks sem sobreviventes.
- Filtro por tipo seleciona grupos-célula inteiros (o grupo é do tipo).
- **Vazio com busca**: "Nada em {mês} para "{q}". Limpe a busca ou troque o filtro."
- **Vazio com filtro**: "Nenhum lançamento de {tipo} em {mês}." — no filtro Diário em
  modo cartão, acrescenta "No modo cartão, o variável vive nas faturas."
- Estados de carga/erro migram para `EmptyState` (skeleton anuncia `status`, erro
  anuncia `alert` com retry) — o skeleton artesanal atual morre.

## O que morre

- O seletor "Por mês / Linha do tempo": a tela vira mês-escopada com `MonthNav` — todo
  lançamento segue alcançável navegando meses; o banner de futuros vira o `Disclosure`
  do mês corrente.
- O botão "Novo" da toolbar: registrar é ação de primeira classe do shell (FAB · CTA
  da sidebar · tecla N), nunca duplicada por tela.
- A expansão "N itens" como única visão da nota: os itens são linhas de primeira classe.

## Backend (uma adição de leitura, TDD)

`TransactionRow.has_refund_link: bool` — verdadeiro quando (a) a Entrada tem
`refund_invoice_id`, ou (b) a despesa tem `invoice_id` cuja fatura tem expectativa de
reembolso (`EXISTS` de Entrada vinculada). Habilita a pílula "Reembolso" sem N+1.

## Motion

Coreografia de entrada única por montagem (fade+translate curto com stagger, CSS puro,
`prefers-reduced-motion` respeitado). Superfícies transitam; **dinheiro nunca anima**.
O bottom sheet abre/fecha com a transição do dialog nativo (mesma gramática do Compose).

## Acessibilidade

- Hierarquia: o título da tela é do shell; h2 por dia (daymark), h3 no cabeçalho de
  célula, grupos-célula como listas (`ul/li`).
- Ordem do DOM = ordem de leitura em todos os viewports (a colunização é só CSS).
- Painéis expansíveis com `aria-expanded`; sheet com `role="dialog"` nativo, foco
  gerenciado, Esc/backdrop fecham.
- Texto e `aria` dizem o valor verdadeiro; contraste AA nos 2 temas × 6 acentos.

## Fora de escopo

- Editar/excluir item individual (reescrever a nota) — contrato de escrita futuro.
- Peças por lançamento na barra do check-in da Hoje (dependia desta lista; segue adiada
  por decisão própria).
- Vínculo reembolso↔item de nota importada (exige casamento item↔fatura por alias).
- Drill de fatura e write-back (tela Cartões).

## Aceitação

1. Daymarks por dia com cabeçalho de célula (total como autoridade), selo "Com
   diferença" e linha sintética de reconciliação no caso real (célula 8.101,58 × nota
   8.101,28 → linha de R$ 0,30) — nunca como item.
2. Desktop escaneia em colunas de verdade (nome | contexto | valor); mobile empilha
   nome/contexto com valor à direita; pílulas de metadado junto do nome, coluna do
   dinheiro só com dinheiro.
3. Busca ao lado do título no desktop e no rodapé da lista no mobile; filtro por tipo
   em chips inline no desktop e bottom sheet no mobile — mesmos resultados nos dois.
4. Helpers de agrupamento/reconciliação puros e testados (vitest); `has_refund_link`
   com TDD no Rust.
5. Gates: `npm run check` verde; react-doctor sem achados novos; e2e visual com
   baselines regenerados do zero; impeccable audit + critique; copy sentence-case;
   dinheiro tabular e estático.
