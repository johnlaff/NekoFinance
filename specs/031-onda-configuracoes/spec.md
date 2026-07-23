# Spec 031 — Onda Configurações: veredito de confiança e quatro seções

## Contexto

A tela Configurações lista cards funcionais (planilha, import, sincronização, bolsos,
lembrete, teto, privacidade, aparência, dados) sem hierarquia editorial. A direção da
identidade redefine a tela: ela abre com um **veredito de confiança** — "Tudo neste
dispositivo" + linha de estado viva da conexão — e organiza tudo em quatro seções
(**Conexão · Privacidade · Aparência · Rotina**), com a gramática de card da direção
(sec-head com ícone + linhas divididas por hairline). Nenhuma capacidade atual é
removida: superfícies densas (painel da planilha, import) recolhem atrás da porta
"Gerenciar"; o restante vira linha.

Zero mudanças de backend: o timestamp da última sincronização (`last_sync_at`), o
estado do write-back (`useWriteBackPending`) e o teto (`get_daily_budget`) já existem.

## Estrutura da tela (ordem do DOM = ordem de leitura)

1. **Greet veredito-primeiro** (`data-large-title`, appbar mobile silencia): `h1`
   "Tudo neste dispositivo" + pílula de estado. Sem brand-mark no conteúdo (regra do
   #198: o glifo vive só no chrome; greet tipográfico).
2. **Conexão** — Google Sheets, escrita só com aprovação, superfícies de pendência,
   porta "Gerenciar".
3. **Privacidade** — dados locais, backup, Mia local, token local.
4. **Bolsos** — gestão das contas de reserva (fora do protótipo; ver Desvios).
5. **Aparência** — tema, animações, diagnóstico, cor de destaque.
6. **Rotina** — lembrete diário, teto do diário.
7. **Rodapé quieto** — "Neko Finance v{versão} · Tauri desktop" (+ aviso de preview
   web quando fora do Tauri).

## Pílula de estado do greet (lógica pura, TDD)

`configView.ts` exporta `greetState(auth, pendingCount, conflictCount, syncLabel)`;
prioridade do pior estado primeiro — o veredito sabe dizer má notícia com o mesmo peso:

| Condição                     | Ponto (cor de status, nunca acento) | Texto                                             |
| ---------------------------- | ----------------------------------- | ------------------------------------------------- |
| `auth === "loading"`         | `--text-faint`                      | Verificando conexão…                              |
| desconectado, `pending > 0`  | `--warning-400`                     | **Desconectado** · N mudanças aguardando          |
| desconectado                 | `--warning-400`                     | **Desconectado**                                  |
| conectado, `conflictCount>0` | `--warning-400`                     | **Conectado** · Conflito de importação a resolver |
| conectado, `pending > 0`     | `--success-400`                     | **Conectado** · N mudanças aguardando             |
| conectado, com `syncLabel`   | `--success-400`                     | **Conectado** · Sincronizado {há X}               |
| conectado, sem timestamp     | `--success-400`                     | **Conectado**                                     |

Singular: "1 mudança aguardando". O rótulo de recência reutiliza `syncRecencyLabel`
(extraída de `AppShell.tsx` para `src/lib/syncRecency.ts` — uma fonte só; o rodapé da
sidebar continua consumindo a mesma função).

## Seções

### Conexão (sec-head: ícone planilha · porta "Gerenciar")

- **Google Sheets** — sub: "Conta conectada · Aba {sheetName}" (conectado; sem aba
  mapeada, só "Conta conectada") / "Verificando conexão…" / "Desconectado". Direita:
  botão ghost "Reconectar" (fluxo OAuth novo — necessário mesmo "conectado" quando o
  refresh token morre).
- **Escrita só com aprovação** — sub: "Nenhuma mudança na planilha sem seu OK".
  Direita: pílula "Sempre" (fato, não toggle — ver Desvios).
- **Pendências visíveis sem abrir a porta**: `ConflictGate` e `WriteBackPending`
  renderizam no corpo do card quando têm conteúdo (ação nunca escondida atrás de
  disclosure).
- **Porta "Gerenciar"** (botão no sec-head, `aria-expanded` + `aria-controls`,
  animação `grid-template-rows 0fr→1fr`, fechada por padrão): `GoogleSheetsPanel` +
  linha de import `.xlsx` (`LocalXlsxImport`). A nota "Nenhuma alteração pendente…"
  morre — o greet já diz "Sincronizado há X".

### Privacidade (sec-head: escudo)

- **Seus dados** — sub: "Guardados só neste aparelho — nada de uso é enviado." +
  linha mono truncada com o caminho do banco. Pílula "Local".
- **Backup do banco** — sub atual; botão "Fazer backup".
- **A Mia responde local** — sub: "Sua planilha não vai para a nuvem". Pílula "Local".
- **Conta Google** — sub: "Token no chaveiro do sistema". Pílula "Local".

### Bolsos (sec-head: cofre)

`PocketsCard` + `PocketsManager` no corpo, sem mudança funcional.

### Aparência (sec-head: paleta)

- **Tema escuro** — Switch (`aria-checked` = tema escuro ativo). Troca pelo MESMO
  caminho do shell: store único de `ThemeToggle.tsx` (hook `useThemeSwitch` exportado
  de lá) + reveal circular a partir do controle; reduced-motion/animações-off →
  instantâneo.
- **Animações** — Switch + sub-diagnóstico vivo (copy atual preservada).
- **Diagnóstico de animações** — linha atual (`MotionDiagnostics`) na gramática nova.
- **Cor de destaque** — sublabel "A cor que o app usa nos seus destaques" +
  "Como funciona?" (`InfoPopover`: separação dura entre acento e cores de status do
  método). Swatches circulares 28px (`aria-pressed`, nome acessível, foco visível,
  alvo ≥ 44px no mobile via pseudo-elemento).

### Rotina (sec-head: sino)

- **Lembrete diário** — Switch (substitui o `SegmentedControl`); sub atual + aviso
  de agendamento do SO inline quando falhar.
- **Horário** — só com lembrete ligado; input `time` estilizado como pílula.
- **Teto do diário** — sub com o dado vivo ("Teto estipulado: R$ X por dia" / "Sem
  teto estipulado"; título neutro enquanto carrega — sem negativo fabricado).
  Direita: botão "Abrir →" com `aria-label` "Abrir teto do diário"
  (`navigate("teto")`).

## Componentes e mudanças transversais

- **`Switch` nasce no design system** (`design-system/components/Switch.tsx`):
  `role="switch"`, trilho pill 40×23, thumb por `transform` (nunca layout), ligado =
  `--accent`/`--accent-ink`. **Desligado corrige o achado do #199**: thumb em tinta
  visível (não branco-sobre-claro) e trilho com borda — contraste ≥ 3:1 verificado
  numericamente nos dois temas. `SettingsScreen` migra `.sw` para ele.
- `syncRecencyLabel` extraída para `src/lib/syncRecency.ts` (AppShell e greet
  consomem a mesma função).
- `useThemeSwitch` exportado de `ThemeToggle.tsx` (o store de módulo continua único).
- `config.css`: estilos novos 100% namespaced `.config__*`. Os blocos legados
  (`.xs`, `.card`, `.cfg-*`, `.sw`) permanecem — TagsScreen e CopilotScreen ainda os
  usam; morrem nas ondas dessas telas.

## Desvios do protótipo (com porquê)

1. **"Escrita só com aprovação" é fato, não toggle.** O protótipo mostra um switch
   ligado; o motor não tem caminho de escrita sem aprovação (invariante do produto).
   Um switch que não desliga é mentira de UI — vira pílula "Sempre".
2. **Bolsos ganha card próprio.** Função existente fora do escopo do protótipo; a
   tela nunca perde capacidade. Mesma gramática visual.
3. **Backup, caminho do banco e telemetria entram em Privacidade**; versão vira
   rodapé quieto. O protótipo não os cobre; Privacidade é o lugar semântico.
4. **Lembrete com horário em linha própria** (protótipo mostra pílula estática
   "21:00"): o horário é editável de verdade; input visível só com lembrete ligado.
5. **Raio de card segue o contrato de token** (`--radius-md`), não os 20px do
   protótipo (regra 12 do ui-standards).

## A11y e motion

- Switches `role="switch"` + `aria-checked`; porta com `aria-expanded`/`aria-controls`;
  `h1` único no greet; swatches com nome acessível; foco visível em todo controle.
- Motion 150–250ms `cubic-bezier(.2,0,0,1)`; reduced-motion instantâneo; dinheiro
  (teto no sub) nunca anima; toggle anima só `transform`/cores.

## Testes e gates

- **TDD**: `configView.test.ts` (tabela de estados do greet, singular/plural,
  prioridade pior-primeiro), `Switch.test.tsx` (role, aria, teclado, contraste de
  classe), `syncRecency.test.ts` (extração preserva casos), `SettingsScreen.test.tsx`
  atualizado (IA nova, porta, fato "Sempre").
- **e2e**: `Configuracoes-{dark,light}` regenerados do zero (2×); novos
  `mobile-config-{dark,light}`; spec do Teto continua navegando pela tela (seletor
  atualizado se a copy mudar).
- Gates da onda: `npm run check`, React Doctor sem novas violações, impeccable
  `audit` + `critique`, revisão adversarial multi-lente antes do PR, CI verde.
