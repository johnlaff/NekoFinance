# Spec 025 — Estados de dado ausente + modo de gasto (motor + primitivas)

## Contexto

Toda régua do método julga hoje sobre números coagidos: campo ausente vira `0`,
zero legítimo vira alerta, e um número derivado (a média do mês anterior como
teto) é exibido como se fosse escolhido. A planilha real demonstra três
semânticas de zero que o app não distingue: **zero-modo** (Diário zerado porque
o gasto variável vive nas faturas de cartão), **zero-placeholder** (itens
`R$ 0,00` em notas de meses futuros pré-lançados) e **zero-pré-história**
(células anteriores à adoção da planilha, avaliadas como `0` pelo template).

Esta spec aterrissa o contrato decidido na issue #178 e executado pela #192:
três estados epistêmicos por régua, modo de gasto global com detecção
automática, estimativa marcada como terceiro tipo de número, e as duas
semânticas de zero que se resolvem no import.

## D1 — Ontologia: três estados epistêmicos + estimativa marcada

Toda régua do método é julgada em um de três estados, com um quarto tipo de
número transversal:

| Estado                          | Significado                                | Apresentação                                                                       |
| ------------------------------- | ------------------------------------------ | ---------------------------------------------------------------------------------- |
| **Veredito**                    | Dado registrado/escolhido presente         | Número pleno, cor de status do método                                              |
| **Zero-diagnóstico**            | Insumo presente e valor legitimamente zero | Ramo próprio com palavra dedicada — nunca escondido, nunca alarme fabricado        |
| **Sem registro**                | Lacuna — o insumo não existe               | Nunca vira número: travessão + rótulo + popover didático com CTA                   |
| **Estimativa** (tipo de número) | Derivado de dado indireto                  | Número com selo explícito "Estimativa" + popover do ritual que o tornaria veredito |

Regra dura: **nenhum número derivado de campo vazio aparece sem marcação**.
Veredito só nasce de dado registrado ou escolhido pelo dono.

No domínio (Rust), cada régua exposta ao front carrega o estado
explicitamente no DTO (campo `state`, serde `snake_case`), nunca sentinelas
numéricos. No front, as primitivas do D7 apresentam cada estado.

## D2 — Modo de gasto global (débito × cartão)

Conceito global, ortogonal aos estados: re-roteia quais insumos alimentam as
réguas do dia **antes** do julgamento. Sem configuração — detecção automática
pura sobre os próprios dados, com histerese.

### Detecção (núcleo puro, TDD)

`detect_spending_mode(samples) -> SpendingModeInfo` em
`src-tauri/src/forecast/mod.rs`, onde `samples` são os **2 últimos meses de
calendário completos + o mês corrente** (ordem cronológica), cada um com:

```rust
pub struct MonthSpendSample {
    /// Dias distintos com Diário realizado (> 0) no mês.
    pub daily_days: u32,
    /// Total de Diário realizado no mês (magnitude, cents).
    pub daily_total_cents: i64,
    /// Existe evento Cartão (realizado ou projetado) no mês.
    pub cartao_present: bool,
}
```

Regras (limiar de ruído `DAILY_NOISE_CENTS = 5_000` = R$ 50,00; constância
`DAILY_ACTIVE_MIN_DAYS = 4`):

- `diario_ativo(m)` = `daily_days >= 4` **e** `daily_total_cents > 5_000`.
- **Débito** se qualquer mês da janela é `diario_ativo` — a constância do
  gesto diário vence na hora (a migração cartão→débito flipa sozinha assim que
  o débito ganha regularidade).
- **Cartão** se nenhum mês é `diario_ativo` **e** algum mês da janela tem
  `cartao_present` — o Diário morto com fatura viva é o perfil que o método
  reconhece. Um lançamento avulso (1 dia, ou total ≤ ruído) não flipa: continua
  não-ativo.
- **Débito** como default quando não há fatura na janela (usuário novo,
  dado insuficiente) — o gesto-base do método.

A histerese é assimétrica por construção: entrar no modo cartão exige a janela
inteira sem constância de débito (≥ 2 meses completos); voltar ao débito exige
apenas um mês com constância. Não há estado persistido — o modo é função pura
da janela.

### Legitimidade do modo cartão (gate)

O modo cartão detectado carrega o gate canônico do método: **economia 20–30%
viva**. `card_gate` no payload:

- `alive` — Economizado% anual realizado (régua do D4) ≥ 20%;
- `below` — < 20%;
- `unknown` — sem renda realizada no ano (régua inativa).

O gate **não** desliga o modo (a detecção é factual); ele muda a leitura: com
`below`, o contexto de modo mostra atenção — o método só considera o crédito
legítimo com a economia viva.

### Efeitos nesta entrega

- `DashboardSummary` ganha `spending_mode: SpendingModeDto { mode, card_gate,
window_months }`.
- No modo cartão, a tela Hoje deixa de fingir régua verde de Diário: o
  check-in/velocímetro re-roteia para as faturas — exibe o Cartão do mês
  (realizado + projetado) e o próximo vencimento em vez de `R$ 0 de R$ X`;
  o teto estipulado permanece visível como referência.
- Chip de contexto de modo (D7) com popover explicando **o que foi detectado e
  por quê** (janela, sinais) — didática, não configuração.
- A aplicação plena no velocímetro redesenhado é da Onda Hoje (#187); aqui
  entrega-se o motor + o re-roteamento honesto da superfície atual.

## D3 — Teto diário: procedência explícita (o fallback silencioso morre)

`effective_daily_ceiling` hoje devolve um `i64` que mistura três origens
indistinguíveis. Passa a devolver procedência:

```rust
pub struct DailyCeiling {
    pub per_day_cents: i64,
    /// chosen | proposed | estimate | none
    pub source: CeilingSource,
}
```

- **`chosen`** — orçamento explícito ativo (`daily_budget.amount > 0`). Único
  caso que é Veredito.
- **`estimate`** — sem orçamento: a média do Diário do mês anterior continua
  exibida, mas **como estimativa marcada** (selo + popover com CTA para
  estipular). O motor de projeção continua usando o valor (a projeção assume o
  gasto típico); o que morre é a exibição sem marca.
- **`none`** — sem orçamento e sem mês anterior com Diário. Sem registro
  (travessão + CTA da cerimônia guiada). A projeção não injeta diário
  (comportamento atual de teto 0 preservado).

A proposta pendente da cerimônia é um **overlay**, não uma procedência do
número exibido: `ceiling_proposal_pending` acompanha a leitura e a UI mostra o
banner de confirmação por cima de qualquer estado sem-veredito — o valor
proposto nunca entra no progresso/projeção antes do aceite explícito.

### Leitor da cerimônia documentada (import)

A planilha real documenta a cerimônia do teto em **notas de células da coluna
Diário** (célula frequentemente vazia/zerada — portanto o leitor varre a grade
de notas diretamente, independente de a célula ter virado transação):

```
Mensal⇥R$ 300,00⇥Transporte
Mensal⇥R$ 200,00⇥Farmácia
...
Total = R$ 1250,00
R$ 1250,00 / 31 Dias = R$ 40,33
```

Parser puro `parse_ceiling_ceremony(note: &str) -> Option<CeilingCeremony>`
(TDD), tolerante a `\t` ou espaços múltiplos:

- Itens: `Mensal R$ <valor> <categoria>` (prefixo `Mensal` opcional em linhas
  subsequentes; valor com vírgula decimal, pontos de milhar opcionais).
- Total: `Total = R$ <valor>` (validado contra a soma dos itens quando ambos
  presentes; divergência → nota rejeitada, sem proposta silenciosamente errada).
- Divisor: `R$ <total> / <N> Dias = R$ <per_day>` — o divisor faz parte da
  cerimônia (a nota real diz `/ 31 Dias` mesmo em meses de 30 dias); o
  `per_day` declarado é recomputado e a nota só é aceita se bater (tolerância
  de 1 centavo de truncamento).

O import varre as células da coluna Diário de cada bloco mensal; a ocorrência
**mais recente** (maior ano-mês) vence. O resultado persiste em:

```sql
CREATE TABLE ceiling_proposal (
    id             TEXT PRIMARY KEY NOT NULL,
    note_hash      TEXT NOT NULL,      -- sha256 da nota normalizada; identidade da proposta
    per_day_cents  INTEGER NOT NULL,
    divisor_days   INTEGER NOT NULL,
    items_json     TEXT NOT NULL,      -- [{name, amount_cents}] na ordem da nota
    source_month   TEXT NOT NULL,      -- "YYYY-MM" da célula onde a nota mais recente vive
    status         TEXT NOT NULL CHECK(status IN ('pending','accepted','dismissed')),
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    resolved_at    TEXT
);
```

- Import upserta por `note_hash`: proposta nova só quando a nota **muda**
  (dispensada não re-propõe; aceita não re-propõe).
- **Aceitar** (confirmação explícita na UI) grava `daily_budget` (per-day) +
  `daily_budget_category` (itens mensais) + `divisor_days`, via a transação
  atômica existente, e marca `accepted`.
- **Dispensar** marca `dismissed` e o teto segue a cadeia (estimate/none).
- Nunca escreve o teto sozinho: **propõe com confirmação explícita** — regra
  do método de que veredito é dado escolhido.

### Divisor persistido

`daily_budget` ganha `divisor_days INTEGER` (nullable; migração aditiva).
`NULL` = teto informado direto por dia (fluxo atual preservado). Com valor, o
editor deriva `per_day = soma_mensal / divisor` via `monthly_to_daily_rate`
(núcleo puro já existente, que finalmente ganha caller de produção).

### Editor de teto em tela própria + cerimônia guiada

Nova screen `teto` ("Teto do diário") registrada no shell (união `Screen` +
`SCREEN_META`; fora de `NAV_ITEMS`/`DOCK_KEYS` — alcançada pelo CTA do tile do
teto na Hoje e por link em Configurações):

- **Com teto ativo**: itens mensais (editor no padrão `DiarioCategorySection`,
  que migra para cá) + divisor + per-day derivado ao vivo; salvar usa
  `upsert_daily_budget_with_categories` estendido com divisor.
- **Com proposta pendente**: banner da proposta (valor, itens, mês de origem)
  com "Usar este teto" / "Agora não" — aceite explícito.
- **Sem nada**: cerimônia guiada de criação — passo a passo: categorias com
  estimativas (itens) ou valor direto por dia; didática do ritual no padrão
  InfoPopover.
- As seções de teto de Configurações são substituídas por um cartão-resumo com
  link para a tela (uma fonte de edição só).

## D4 — Economia: veredito só de economia registrada; previdência condicional

- A régua Economizado% (anual e mensal) continua alimentada por economia
  **registrada** (eventos Economia + anotação da aba, `max(derivado, anotado)`
  — economia é ato, não resíduo).
- **Previdência condicional à reserva líquida**: quando a reserva (D5) ≥ 6
  meses, os eventos Patrimônio passam a contar na régua de economia (o método:
  primeiro constrói liquidez, depois patrimônio conta como poupança); < 6
  meses, ficam fora (comportamento atual). O popover expõe as duas leituras
  (com e sem previdência). A inclusão vale para a régua E para o guardrail de
  poupança do "pode gastar hoje" (uma régua só, sem bifurcar semântica).
- **Estados**: veredito quando a régua (economia registrada + previdência
  condicional) > 0 — previdência contando é dado registrado, só vive noutro
  balde; senão **sem registro** — o app exibe a **sobra derivada (Colchão)**
  como estimativa marcada + CTA didático do ritual de transferir para a
  reserva.
  A economia NÃO tem ramo de zero-diagnóstico: a planilha real demonstra que
  zero explícito e célula vazia são intercambiáveis na prática do dono (e o
  import da aba Economia já normaliza ambos), então distinguir "guardei 0" de
  "não uso este balde" pela tipografia da célula seria uma mentira.

## D5 — Reserva: retrato vivo com poucos meses

`reserve_months` deixa de fabricar `0.0`:

- **Veredito** — baseline = mediana de 6 meses completos (janela cheia).
- **Estimativa ("retrato vivo")** — 1–5 meses com custo de vida na janela: a
  mediana do que existe vale como retrato, com selo de estimativa (o método
  não exige histórico mínimo).
- **Zero-diagnóstico** — contas de reserva existem com saldo 0 ("Sem reserva",
  palavra dedicada; o alerta é legítimo).
- **Sem registro** — sem contas de reserva marcadas ou sem baseline nenhum:
  travessão + CTA (mapear bolsos / importar).

`DashboardSummary.reserve_months` vira `reserve: ReserveReadingDto { state,
months, basis_months }`.

## D6 — Import: as duas semânticas de zero que não são estado de UI

- **Pré-história**: `store_balance_series` corta os **zeros à esquerda** — dias
  de saldo 0 em meses anteriores ao primeiro mês com qualquer transação
  importada ou saldo ≠ 0. Esses meses ficam sem linha (Sem registro honesto)
  em vez de seis meses de "saldo zero" fabricado pelo template da planilha.
  Zeros de saldo **após** a adoção continuam dados reais.
- **Placeholder**: itens `R$ 0,00` em notas de linhas **projetadas**
  (`is_projection = 1`) passam a persistir como `line_item` com
  `amount_cents = 0` — a estrutura pré-lançada do futuro fica visível ("a
  preencher") sem virar valor. Em linhas realizadas, zero continua descartado
  (ajuste/ruído). Agregações não mudam (0 soma 0); o round-trip de write-back
  preserva as linhas.

## D7 — DS: primitivas canônicas de estado (Midnight Purr)

Grupo novo em `tokens/states.css` (aliases theme-aware, padrão do arquivo):

```css
/* ---- Estados epistêmicos (dado ausente / estimado / zerado) ---- */
--state-estimate: var(--info-400); /* selo "Estimativa" — informa, não alarma */
--state-no-record: var(--text-faint); /* travessão + rótulo "Sem registro" */
--state-zero: var(--text-muted); /* palavra dedicada de zero-diagnóstico */
```

Componentes novos em `src/design-system/components/` (padrão da casa: export
nomeado, teste irmão, inline-style com tokens; sem cor sozinha — sempre
palavra/ícone):

- **`EstimateMark`** — selo "Estimativa" inline junto ao número (tipografia
  pequena, `--state-estimate`), com `InfoPopover` do ritual correspondente
  (copy por régua). Dinheiro continua tabular e nunca anima.
- **`NoRecordDash`** — travessão (`—`) + rótulo "Sem registro" +
  `InfoPopover` didático com CTA (ação por régua: estipular teto, mapear
  reserva, registrar economia).
- **`ModeChip`** — contexto de modo de gasto ("Modo cartão" / "Modo débito")
  com `InfoPopover` explicando a detecção; no `card_gate = below`, para o
  chip com palavra+ícone de atenção (cores de status do método, nunca o
  acento).
- Palavra dedicada de zero-diagnóstico por régua (copy, não componente):
  "Zerado" como default; "Sem reserva" na reserva. Sentence-case sempre.

## Fora de escopo

- Domínio do cartão (#179): múltiplos cartões, fatura como entidade, FK
  transação→cartão. A detecção de modo usa apenas os sinais existentes
  (eventos Cartão via seção de nota / `payment_method`).
- Velocímetro redesenhado e aplicação visual plena na Hoje (#187) — consome o
  motor daqui.
- Ondas Lançamentos–Mia (#188–#191) — consomem as primitivas do D7.
- Write-back do teto para a planilha (o teto estipulado vive no app).
- Backfill/import de histórico do teto além da proposta mais recente.
- Placeholder em célula projetada de valor TOTAL zero: sem valor, a célula não
  materializa linha e a nota não é lida — limitação conhecida; a estrutura de
  fatura por célula é do domínio do cartão (#179).

## Aceitação (critérios do ticket #192 + gates)

1. Cada régua exposta (teto, reserva, economia) tem estado definido para dado
   ausente, distinto de zero — verificável nos DTOs e na UI.
2. A tela Hoje tem veredito coerente quando falta o teto estipulado: proposto
   (com confirmação), estimativa marcada ou travessão+CTA — nunca `R$ 0,00`
   fabricado; no modo cartão o check-in re-roteia para as faturas.
3. Nenhum número derivado de campo vazio aparece sem marcação (selo de
   estimativa transversal).
4. Detecção de modo: TDD com tabela de janelas (avulso não flipa; migração
   flipa ao ganhar constância; default débito).
5. Leitor de cerimônia: TDD com a gramática real (tabs, `/ 31 Dias` fixo,
   total divergente rejeita); proposta re-emerge só quando a nota muda.
6. Import: TDD pré-história (zeros à esquerda caem; zero pós-adoção fica) e
   placeholder (zero projetado persiste; zero realizado não).
7. Gates: `npm run check` verde, e2e visual smoke, react-doctor zerado,
   impeccable audit + critique nas superfícies novas, copy sentence-case.
