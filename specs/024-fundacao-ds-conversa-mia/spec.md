# Spec 024 — Fundação do design system "Midnight Purr" (tokens + shell)

## Contexto

A identidade visual do app foi rediscutida por inteiro sob a marca existente. A direção
vencedora — **"Conversa com a Mia"** — define uma malha visual zinc dark-first com acento
configurável, tipografia Geist, geometria de pílula e um shell nativo por viewport. Esta
spec sistematiza essa direção como o design system **"Midnight Purr"** e aterrissa a
fundação: tokens novos + AppShell redesenhado num único PR que troca o app inteiro de uma
vez — sem convivência de duas paletas.

As telas continuam com seus layouts atuais nesta entrega; elas herdam a nova malha via
tokens. De-carding, assinaturas visuais por tela e coreografias de motion são ondas
posteriores.

## Decisões

### D1 — Nome e identidade do DS

- O DS chama-se **"Midnight Purr"**: mantém a linhagem dark-first e assume a assinatura
  felina da marca. Substitui "Midnight Ledger" em toda referência (PRODUCT.md, AGENTS.md,
  skill, DS canônico).
- Marca preservada: nome Neko, gato como logo e avatar da Mia, vocabulário do método,
  veredito-primeiro, honestidade epistêmica.

### D2 — Malha de cor em duas camadas

**Camada 1 — primitivos** (só `tokens/colors.css` os conhece):

- Neutros zinc (croma zero). Dark: `#09090b` fundo · `#18181b` superfície · `#1f1f23`
  superfície-2 · `#27272a` borda/superfície-elevada · `#71717a` borda de controle (≥3:1) ·
  `#fafafa` texto · `#a1a1aa` muted · `#8f8f99` faint (AA). Light: `#fafafa` fundo ·
  `#ffffff` superfície · `#f4f4f5` superfície-2 · `#e4e4e7` borda · `#71717a` controle ·
  `#18181b` texto · `#52525b` muted.
- **6 paletas de acento** via `[data-accent]` no `:root`, cada uma com par `--accent` +
  `--accent-ink` (tinta sobre o acento) por tema:
  jade (default) · lima · violeta · âmbar · céu · rosa. No claro, o acento escurece o
  suficiente para AA como texto.
- **Separação dura entre cor de marca e cor de status do método**: `--ok`/`--warn`/
  `--danger`/`--info` são fixos por tema e NUNCA seguem a paleta. Paz nunca muda de cor
  com o acento; reembolso é verde em toda paleta; dinheiro positivo/negativo idem.

**Camada 2 — aliases semânticos** (a interface que o app inteiro consome — nomes
preservados): `--bg`, `--bg-subtle`, `--surface`, `--surface-2`, `--surface-elevated`,
`--border`, `--border-strong`, `--border-input`, `--text`, `--text-strong`,
`--text-muted`, `--text-faint`, `--primary`, `--text-on-primary`, `--primary-quiet`,
`--focus-ring`, `--money-pos`, `--money-neg`, tints de status, série de gráfico. Trocar os
valores por baixo desses nomes é o que permite o app inteiro virar de uma vez.

- `--primary` → acento da paleta ativa; `--text-on-primary` → `--accent-ink`.
- Brass deixa de existir como secundária de marca. Os papéis que o brass ocupava
  (pendente, projetado, atenção) passam ao `--warn`. `--secondary`/`--secondary-quiet`
  tornam-se aliases do warn até as ondas removerem os usos.
- Gráficos usam família própria fixa (dado, não marca): não seguem o acento.
- Cores categóricas (tags/séries) re-harmonizadas para a malha zinc, fixas por tema.

### D3 — Tipografia

- `--font-sans` → **Geist** (variable, self-hosted, OFL — mesma família da Geist Mono já
  vendorada). Hanken Grotesk sai.
- **Dinheiro em Geist com `tabular-nums`** (`--font-money` deixa de ser mono): peso
  semibold/bold, algarismos tabulares e lining — a propriedade de alinhamento é
  inegociável; a família muda. O átomo `Money` continua o único caminho para valor
  monetário; valor nunca anima.
- Geist Mono permanece (`--font-mono`) para parcelas `2/10`, código e citações.
  Newsreader permanece a serifa editorial.
- Rótulos micro-uppercase saem do vocabulário do shell (o DS não oferece mais
  `.t-eyebrow` como idioma de chrome; a classe permanece até as ondas limparem as telas).

### D4 — Geometria e elevação

- Pílula domina: raios sobem — `xs 6 · sm 10 · md 14 · lg 18 · xl 22 · pill 999`.
  O override `.neko-app` de raios em `redesign.css` morre (os valores viram os tokens).
- Sombras discretas; dark apoia-se em borda + lift (`rgba(255,255,255,.03)` dentro de
  card), light em sombra ambiente suave.

### D5 — Shell por viewport (nunca um esticado do outro)

**Desktop (>900px)** — sidebar fixa 236px:

- Marca: gato em círculo de acento + "Neko".
- CTA primário "Registrar lançamento" com `kbd N` (fundo de acento) — a ação primária
  mora na navegação.
- Nav plana, sem headers de grupo uppercase: Hoje · Lançamentos · Este mês · O ano ·
  Calendário · Horizonte · Tags · Mia (ícone = gato, o mesmo avatar do chat) ·
  Configurações. Dicas numéricas (`hints`) permanecem.
- Rodapé: alternância de tema (ícone mostra para onde o toque leva) + estado da planilha
  (recência de sync; navega para Configurações).
- Topbar desktop enxuta: só título + crumb da tela. O botão "Lançar", o sino sem função e
  o toggle de tema saem do topo (CTA e tema agora vivem na sidebar).

**Tablet (701–900px)** — a sidebar estreita para trilho de ícones (rótulos ocultos,
`title` nos itens); mesma estrutura, sem esconder destino nenhum.

**Mobile (≤700px)** — chrome de app:

- Appbar sticky com blur: gato + título da tela (+ data como subtítulo no Hoje) + toggle
  de tema + menu "mais" com os destinos fora do dock (O ano · Horizonte · Tags ·
  Configurações).
- **Tab bar flutuante** (não colada na borda, safe-area respeitada) com 5 destinos —
  Hoje · Lançamentos · Este mês · Calendário · Mia — e o **FAB de registrar embutido na
  barra** (não flutuando sobre a lista: em finanças, número tapado é pior que um pixel a
  menos de folga). Aba ativa vira pílula de acento que abraça ícone + rótulo.
- Barra encolhe ao rolar para baixo e volta ao subir; alvos ≥44px; inputs com
  `font-size ≥16px`.
- **Coordenação large-title**: contrato via `[data-large-title]` — quando a tela marca
  seu título grande, o título da appbar só assume quando o large-title sai de vista
  (IntersectionObserver no shell). Telas atuais não marcam ainda: a appbar mostra o
  título direto; as ondas adotam tela a tela.

**Chrome fixo em navegação**: `view-transition-name` próprio para sidebar, appbar e dock —
a tela anima; o chrome fica parado. O circular reveal do tema permanece.

### D6 — Seletor de acento

- `data-accent` persiste em `localStorage` e aplica no `:root`, mesmo mecanismo do tema.
- Configurações → Aparência ganha um seletor de 6 amostras (uma por paleta), com nome e
  `aria-pressed`; default jade.
- Default **jade**: é a cor consolidada da marca — a paleta viva (lima etc.) é escolha do
  usuário, não a cara de fábrica. A geometria da lane de referência é régua, não
  identidade; o acento de fábrica idem.

### D7 — Contrato de variantes

Variantes são declaradas, nunca herdadas por acidente: componente novo do DS declara suas
variantes por prop/atributo explícito; estilo contextual (`.tela .componente`) não cria
variante nova. Vale como regra de guideline do DS a partir desta entrega; a varredura dos
casos legados é trabalho das ondas.

## Fora de escopo (ondas por tela — fog do mapa)

- De-carding, assinaturas visuais proprietárias aplicadas, coreografias de motion por
  tela, empty states com o gato, recibo auditável da Mia, régua/termômetro redesenhados.
- Roving tabindex do calendário (pertence à onda da tela Calendário).
- Remoção dos usos legados de `.t-eyebrow`/micro-labels dentro das telas.
- Haptics nomeados (só fazem sentido no empacotamento mobile futuro).

## Critérios de aceite desta fundação

1. App inteiro renderiza na malha nova nos 2 temas × 6 acentos; nenhuma referência a
   `--jade-*`/`--brass-*`/`--ink-*` fora de `tokens/colors.css` (compat mínima documentada
   se inevitável).
2. Shell novo nos três ranges (desktop/tablet/mobile) com os 9 destinos alcançáveis em
   todos; alvos ≥44px no mobile; `prefers-reduced-motion` respeitado; WCAG AA nos pares
   texto/fundo dos tokens (verificado por cálculo de contraste).
3. Dinheiro: tabular em todo valor; nenhuma animação de valor monetário.
4. Gates verdes: `npm run check`, React Doctor zerado, Playwright visual smoke com
   screenshots inspecionados, impeccable audit + critique.
5. DS canônico (Docs), skill vendorada e sync claude.ai/design atualizados na mesma
   entrega; PRODUCT.md/AGENTS.md referem "Midnight Purr".

## Verificação

- Cálculo de contraste dos pares de token (script one-shot no scratchpad; pares AA
  documentados em comentário nos tokens quando o valor foi ajustado por isso).
- Testes existentes de AppShell/ThemeToggle adaptados ao shell novo; teste novo para o
  seletor de acento (persistência + aplicação no `:root`) e para o menu "mais" mobile.
- Smoke visual Playwright nos dois viewports.
