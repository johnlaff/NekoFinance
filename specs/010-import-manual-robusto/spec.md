# Spec 010 — Import manual robusto (dogfooding-readiness)

> Status: **ativa** (2026-06-12). Fundação que a spec 008 pressupõe mas não cobre: antes de
> automatizar qualquer import, o app precisa funcionar como **leitor fiel da planilha real**,
> com o dono mantendo 100% o preenchimento manual. A spec 008 fica **adiada** até o dogfooding
> desta spec gerar aprendizados (ver nota de status na 008).

## Problema

O dono nunca usou o app conectado à planilha real (dogfooding zero). A análise multi-agente de
2026-06-12 mostrou que o import manual existente está quebrado nos dois caminhos — qualquer
tentativa de dogfooding falharia no primeiro dia:

| #   | Bug                                                                                    | Onde                                       | Efeito com a planilha real                                                                      |
| --- | -------------------------------------------------------------------------------------- | ------------------------------------------ | ----------------------------------------------------------------------------------------------- |
| 1   | Guard `i > 0` ao derivar offsets de mês                                                | `google_sheets/import.rs:179`              | JANEIRO (coluna A, offset 0) é dropado e todo mês desloca 1 para trás                           |
| 2   | Range `'{sheet}'!A:Z` no path Sheets ao vivo                                           | `commands.rs:148`                          | Só 26 colunas — corta JUNHO–DEZEMBRO (a planilha vai até a coluna BO)                           |
| 3   | `parse_number` remove `.` incondicionalmente                                           | `google_sheets/import.rs:252`              | Floats dot-decimal do xlsx (calamine) inflam 100× (`12.34` → R$ 1.234,00)                       |
| 4   | Checksum de **batch inteiro** como dedup                                               | `google_sheets/import.rs:25-113`           | Re-import sem mudança = skip silencioso; com qualquer mudança = **duplica todas as transações** |
| 5   | OAuth sem `access_type=offline`                                                        | `oauth/pkce.rs:63-82`                      | Google não devolve refresh_token → sessão morre em 1 h (`ensure_valid_token` falha)             |
| 6   | Scope só `spreadsheets.readonly`                                                       | `oauth/pkce.rs:75-77` vs `commands.rs:898` | Listagem de planilhas usa Drive v3 sem scope → 403                                              |
| 7   | `.env.example` documenta `GOOGLE_OAUTH_CLIENT_ID`, frontend lê `VITE_GOOGLE_CLIENT_ID` | `GoogleSheetsPanel.tsx:58`                 | Botão Conectar permanentemente desabilitado                                                     |
| 8   | Offsets de mês aceitam qualquer célula não-vazia                                       | `google_sheets/import.rs:178-182`          | Célula espúria entre blocos desloca todos os meses seguintes                                    |

Bug latente registrado, fora do caminho crítico: `safe_to_spend_today_cents` retorna o vale
mínimo do horizonte, não o headroom (`forecast/mod.rs:267`).

> Atualização: o import **já** grava `is_fixed` pela coluna de origem (Saída→fixa, Diário→variável;
> ver `import_sets_is_fixed_for_saida`), então `classify()` separa Saída × Diário corretamente. O
> Livro-razão expõe esse tipo via `MovBadge` (coluna "Tipo"). `payment_method` segue não vindo do
> import (a planilha colapsa cartão em Saída no vencimento), o que é fiel ao método.

## Objetivo (definição de sucesso do dogfooding)

O dono abre o app, conecta **ao vivo** na planilha Google Sheets real (decisão do dono:
prioridade desde o início, sem baixar xlsx), e:

1. importa/re-importa quantas vezes quiser, **sem duplicar nada** (idempotência);
2. vê os 12 meses de 2026 com **data e valor corretos** (incluindo os lumps futuros
   pré-lançados à mão: salário nos dias 28–31, contas fixas);
3. vê o saldo projetado encadeado **batendo com a coluna Saldo da planilha** (em centavos, com
   tolerância explícita), do dia de hoje para frente;
4. **zero escrita** na planilha — o app é leitor nesta fase.

## Princípio-norte: paridade com a planilha + MAIS visibilidade (nunca menos)

Decisão do dono (2026-06-12), elevada a princípio do produto: **o app nunca pode mostrar menos
do que a planilha já mostra.** Toda visão que o dono tem hoje no Sheets deve ser replicada de
forma visual, didática e intuitiva — e o app deve ir além: análises que a planilha não dá de
graça (comparar meses/anos, tendência de gasto subindo/descendo, em que período gastou mais).

O import já preserva **100% dos dados** (transações de todos os meses/anos + a série de Saldo
diário). O risco NÃO é perder dado — é **não ter telas que mostrem o histórico**. Hoje o app só
renderiza o mês corrente (Dashboard + previsão diária do mês), então mostra muito menos que a
planilha. Fechar essa lacuna é trabalho de **views**, não de dados.

Matriz de paridade (visão da planilha → tela do app):

| Visão na planilha                                                      | Tela no app                      | Estado                               |
| ---------------------------------------------------------------------- | -------------------------------- | ------------------------------------ |
| Grade do ano (12 meses, `Data\|Entrada\|Saída\|Diário\|Saldo` por dia) | Livro-razão (slice 8)            | a construir                          |
| Saldo corrente encadeado, dia a dia                                    | coluna Saldo do livro + previsão | série capturada ✓ / view a construir |
| Vários anos (abas 2025, 2026, …)                                       | seletor de ano no livro          | a construir                          |
| Comparar meses/anos, tendência de gasto                                | Análises (slice 9)               | a construir                          |
| Aba Economia (taxa de poupança, régua 20–30%)                          | Economia (slice 7)               | a construir                          |
| Totais / indicadores do mês                                            | Dashboard                        | parcial (mês atual)                  |

**Regra de aceite transversal:** nenhuma feature de import/projeção pode descartar coluna ou
período que a planilha tenha. Quando uma coluna ainda não tem tela (ex.: Diário hoje não vira
evento de gasto futuro), ela é **capturada e guardada** mesmo sem render, para a view chegar
depois sem re-importar.

## Princípios

- Nada de schema novo de automação (`source`/`provider_txn_id`/`invoice` são da 008). A
  identidade de re-import usa o que existe (`sync_log.source_sheet` + `entity_id`).
- Dinheiro em centavos inteiros; comparação com a planilha sempre com tolerância, nunca `==`
  de float (a planilha carrega floats de 4 casas, ex. `5678.1234`).
- A semente do forecast é o **Saldo da planilha no dia mais recente ≤ hoje** (decisão do dono
  2026-06-12: "usar a coluna Saldo da planilha"). É seguro porque a engine só carrega eventos
  com `date > hoje` — os lumps futuros pré-lançados ficam DEPOIS da semente e nunca contam duas
  vezes (o EC14 da 008). Implementado em `import::parse_balance_series` + `projection_seed`
  (precedência: planilha > Bolsos). Sem planilha importada, cai nos Bolsos líquidos (spec 007).
- TDD obrigatório: tudo aqui é finance math ou bug fix (regra do AGENTS.md) — cada bug da
  tabela acima ganha teste de regressão.

## Sequência (vertical slices)

### Slice 0 — Bugs de parsing que corrompem dados em silêncio (TDD)

- **Offsets de mês por nome, não por posição não-vazia**: derivar `month_offsets` das células
  que são **nome de mês** (`is_month_name`/novo `month_number_from_name` em `layout_detect`),
  mapeando o nome → número do mês (JANEIRO→1), com fallback `step_by(block_size)` **a partir
  do offset 0**. Resolve os bugs 1 e 8 de uma vez (JANEIRO em A entra; célula espúria entre
  blocos não desloca nada).
- **Range da grade inteira** no `import_sheet_data`: `'{sheet}'` (grade usada completa) no
  lugar de `'{sheet}'!A:Z`. Preview e detecção continuam com ranges curtos (suficientes e
  mais baratos).
- **Valores numéricos imunes a locale nos DOIS paths**: o path Sheets ao vivo pede
  `valueRenderOption=UNFORMATTED_VALUE` (números crus, nunca a string formatada pelo locale
  da planilha) e o path xlsx usa o `Data::Float` do calamine; ambos normalizam números com
  4 casas fixas (`{:.4}`) antes de virar string — `123.456` vira `123.4560`, eliminando por
  construção a ambiguidade "3 dígitos após o separador".
- **`parse_number` com regra fechada de separador** (defesa para qualquer string que ainda
  chegue formatada):
  - tem `.` e `,` → o que aparece **por último** é o decimal (cobre pt-BR `6.012,73` e
    en_US `6,012.73`);
  - só `,` → decimal, exceto padrão claro de agrupamento de milhar (`6,012`, grupos de 3);
  - só `.` → decimal, exceto padrão claro de agrupamento (`6.012`, `1.234.567`).
- **Dia inexistente no mês não vira transação** (a geometria tem linhas fixas 1–31 em todos
  os blocos; fevereiro 29–31 herda fórmulas — `2026-02-30` é pulado).
- **Blocos de mês deduplicados** (primeira ocorrência vence): anotação solta com nome de mês
  não cria bloco-fantasma.
- Fixtures de teste com a **geometria real suja**: JANEIRO no offset 0, 12 blocos até a
  coluna BO, célula espúria entre blocos, e a bateria de valores representativos
  (`1234.56`, `12.34`, `5678.1234`, `1.234,56`, `1,234.56`, `1.234`, `-45,00`).

### Slice 1 — Re-import idempotente (TDD)

Substituir a semântica "batch novo = INSERT de tudo" por **replace-all por aba**, atômico:

- checksum idêntico ao do último import da aba → no-op (nada mudou);
- checksum diferente → numa única transação SQL: `DELETE` das transações originadas da aba
  (join por `sync_log.entity_id` com `source_sheet`), `DELETE` do `sync_log` da aba,
  re-INSERT completo, `COMMIT`. Falha em qualquer linha → rollback total (nunca estado
  parcial).
- O replace só toca transações **daquela aba** — outras abas e lançamentos manuais futuros
  ficam intactos.
- Trade-off aceito: IDs de transação são regenerados a cada replace. OK enquanto nada
  referencia transação importada (split/invoice são da 008, que trará identidade estável
  por linha).

Testes: re-import idêntico é no-op; re-import com 1 linha a mais não duplica as demais;
linha removida da planilha some do banco; replace da aba "2026" não toca a aba "2025".

### Slice 2 — Conexão ao vivo robusta (OAuth de verdade)

Prioridade do dono: usar a planilha real no Google Sheets desde o início.

Fatos verificados na documentação oficial (2026-06-12, evitando o que o Google deprecou):

- **Loopback `http://127.0.0.1:porta` é o redirect recomendado para "Desktop app"** (está
  deprecado apenas para os tipos Android/Chrome/iOS) — o fluxo existente está no caminho
  suportado. OOB está morto e nunca foi usado aqui.
- **Apps instalados sempre recebem refresh_token** ("refresh tokens are always returned for
  installed applications" — doc native-app). `access_type=offline`/`prompt=consent` são do
  fluxo web e NÃO são adicionados. **Contingência registrada**: se o dogfooding mostrar
  `refresh_token` vazio no token armazenado, adicioná-los em `build_auth_url` (1 linha).
- **`client_secret` é opcional** no token exchange de app instalado; relatos de campo
  divergem, então o app envia o secret **quando configurado** (`VITE_GOOGLE_CLIENT_SECRET`,
  local/gitignored; o secret de Desktop app não é confidencial por definição).
- A criação do OAuth client **não tem caminho por CLI**: o `gcloud iap oauth-clients` é
  deprecado (IAP OAuth Admin APIs desligam em jan–mar/2026), só cria client web e exige
  organização. Habilitar as APIs (`gcloud services enable sheets.googleapis.com
drive.googleapis.com`) funciona por CLI e já foi feito no projeto `neko-finance`.

Implementação:

- Scope `https://www.googleapis.com/auth/drive.metadata.readonly` adicionado para a listagem
  de planilhas (`list_user_spreadsheets`), **e** campo de URL/ID colado como fallback
  (`extractSpreadsheetId` — zero dependência do Drive; a falha do picker não bloqueia).
- `client_secret` opcional propagado frontend → comandos → exchange/refresh
  (`ensure_valid_token`/`refresh_access_token` já existiam em `token_store.rs`).
- Env vars unificadas: `VITE_GOOGLE_CLIENT_ID` + `VITE_GOOGLE_CLIENT_SECRET` (o
  `.env.example` documentava `GOOGLE_OAUTH_CLIENT_ID`, que o código nunca leu).
- `check_auth_status`: expirado **com** refresh_token = "connected" (renova sob demanda no
  próximo uso); "expired" só quando precisa reconectar de verdade.

**Insumos do dono (bloqueia o teste de ponta a ponta, ver §Insumos).**

### Slice 3 — Seed guiado (empty-state)

Quando não há conta `liquid` cadastrada, o Dashboard instrui explicitamente: _"cadastre sua
conta corrente em Ajustes → Bolsos com o **Saldo de HOJE da planilha**"_ — antes de falar em
importar. Texto inequívoco sobre "hoje" (nunca saldo futuro — EC14 mínimo). Sem mudança de
Rust; UI apenas.

### Slice 4 — Reconciliação Saldo planilha × projetado (TDD; o gate de prova)

A prova quantitativa de que o leitor funciona — sem isso o dogfooding é "parece certo no olho":

- ativar a leitura da coluna `Saldo` (mapping `balance`, hoje `is_active=0`) **apenas para
  reconciliação** — nunca para semear nada;
- comando que compara, por dia do mês exibido, o Saldo da planilha vs o `balance_cents`
  projetado, **do dia de hoje para frente** (o motor não projeta o passado, por construção);
- arredondamento 4→2 casas na fronteira; tolerância derivada e documentada (floats de 4 casas
  acumulam resíduo ao longo do encadeamento — derivar o limite do nº de dias, não chutar);
- painel simples mostrando os deltas por dia; delta acima da tolerância é sinal de bug de
  parsing/classificação a investigar.

### Slice 5 — Seletor de mês / horizonte multi-mês (TDD no core)

- `get_forecast` aceita `year`/`month` opcionais; `project()` já é puro e encadeia até
  qualquer `horizon_end`.
- **Decisão de escopo obrigatória**: `deepest_deficit` e `safe_to_spend_today_cents` varrem o
  horizonte inteiro (`forecast/mod.rs:266-267`) — ao alargar o horizonte, fixá-los ao mês
  corrente (sub-slice do daily) ou redefinir explicitamente o significado na UI. Sem isso o
  seletor de mês corrompe o alerta principal do Dashboard.
- Dashboard: prev/next + "voltar para hoje".
- Aproveitar para corrigir a semântica de `safe_to_spend_today_cents` (bug latente).

### Slice 6 — Auditabilidade do import

- Descrições com data real (`Entrada 28/jan`) no lugar de `Entrada {aba}` hardcoded
  (`import.rs:227,241`).
- Sumário pós-import: contagem de Entradas/Saídas por mês + linhas descartadas, exibido na
  UI — para o dono bater o olho e perceber um mês vazio por bug de geometria.

### Slice 7 — Aba Economia como métricas (não transações)

**Contexto (metodologia, aula 3 do curso da planilha):** a aba `Economia` é, no material do
método, "a métrica mais importante" — o painel da taxa de poupança (régua 20–30%). Layout por
ano: `mês | Entradas | Economia | %`, onde `Entradas` é fórmula que puxa o total do mês da
aba-ano (`'2026'!B38`), `Economia` é **manual** (quanto poupou) e `%` = Economia/Entradas.
Não tem transações; é agregado mensal + um número manual.

**Estado atual (2026-06-12):** importá-la como transações produziria lixo, então ela está
**bloqueada nos dois caminhos de import** — UI (seletor desabilita abas de métricas,
`src/lib/sheet-tabs.ts`) e backend (`layout_detect::is_metric_tab` em `import_sheet_data` e
no loop do xlsx). A detecção estrutural também a rejeita (meses um por linha, nunca ≥2 na
mesma linha) — teste `economia_layout_fails_structural_detection`.

**O que falta (este slice):**

- Parser dedicado da aba Economia: por linha-mês, extrair `Economia` (manual). `Entradas` e
  `%` o Neko **deriva** dos dados já importados das abas-ano — importar só o que não é
  derivável; reconciliar o derivado contra o lido como check de integridade.
- Modelo: tabela `savings_entry` (ano, mês, valor poupado) ou equivalente — decidir junto
  com o wire de `reserve`.
- Dashboard: taxa de poupança mensal + régua 20–30% (já previsto no design do método e na
  spec 008 §métricas).
- A aba acumula anos (a mesma é reusada ano após ano, ~10 anos de uso real) — o parser deve segmentar por ano.

### Slice 8 — Livro-razão histórico (paridade com a grade da planilha)

A grade ano-a-ano é a visão-assinatura da planilha; hoje o app não a tem. Esta é a entrega que
mais fecha a lacuna "o app mostra menos que a planilha".

- **Dado já existe**: transações de todos os meses/anos + `sheet_daily_balance` (a coluna Saldo
  diário, capturada no import). Nenhum dado novo a importar — é só uma view.
- Tela "Livro" / "Histórico": por ano, uma grade mês a mês com `Data | Entrada | Saída | Diário
| Saldo` por dia, espelhando a planilha mas legível (dark-first, Midnight Ledger). Saldo
  corrente do `sheet_daily_balance`; dias realizados vs projetados distintos visualmente.
- Seletor de ano (2025, 2026, …) — uma aba-ano por seletor, como na planilha.
- Backend: comandos de leitura agregando por (ano, mês, dia) — sem recomputar nada que a
  planilha já tem; o Saldo vem da série importada, não de re-projeção.
- Diário: hoje não é importado como evento (só Entrada/Saída ativos). Para paridade total da
  grade, capturar também a coluna Diário (já existe mapeamento `daily_budget`, `is_active=0`) —
  guardar mesmo antes de virar evento de gasto, pela regra de aceite transversal.

### Slice 9 — Análises (mais visibilidade que a planilha)

O que a planilha não dá de graça e o dono pediu explicitamente: comparar períodos e ver
tendência.

- Comparativo mês a mês e ano a ano: Entradas, Saídas, Diário, performance, taxa de poupança.
- "Em que meses gastei mais" / "meu gasto está subindo ou descendo" — séries temporais desde o
  início do uso da planilha (todos os anos importados).
- Tudo determinístico sobre os dados importados (sem LLM em cima de número — regra do AGENTS).
- Base para a Mia responder perguntas analíticas ("gastei mais este ano que no passado?").

### Slice 10 — Guardrail "pode gastar" fiel ao método (IMPLEMENTADO 2026-06-13)

O "pode gastar até X sem furar o mês" antigo olhava só o vale de caixa do mês corrente — frouxo
para quem tem colchão (o caixa cresce; libera número alto e enganoso). Substituído por um
**guardrail duplo**, o mais apertado de:

1. **Caixa** — `menor saldo projetado no horizonte − piso de reserva` (padrão de mercado:
   Simple/Monarch/Copilot/Rocket descontam compromissos futuros; YNAB reserva no swipe).
2. **Poupança** — `performance do mês − meta×renda` (meta 25%). Negativa = já abaixo da meta →
   pode gastar 0. Espelha o gate determinístico do método: reserva ≥ piso **E** poupança 20–30%.

Horizonte estendido **até o fim dos dados pré-lançados** (a planilha lança o ano inteiro), não o
mês. UI: "pode gastar R$ 0 sem furar a meta do mês", strip de **Performance por mês** (Caixa ≠
Performance, expõe os meses magros), aviso do cartão ("sequestra o salário futuro").
Pesquisa de mercado (3 agentes) + review adversarial (4 lentes) conduzidos. Código:
`forecast::safe_to_spend_today` + `project_with_metrics` + `load_metric_events` +
`commands::reserve_floor`. Verificado: pode-gastar = R$ 0 quando a poupança manda.

**Bug P0 que o review pegou e foi corrigido:** a performance do mês corrente era calculada só
com eventos `date > hoje`, ignorando o realizado do mês → mês com sinal trocado e guardrail
decidindo errado (valor positivo em vez de 0). Fix: `metric_events` = realizado do mês + futuros,
separado do encadeamento de caixa (que continua future-only para não dobrar a semente).

### Slice 11 — Previsibilidade + poupança anual (IMPLEMENTADO 2026-06-13)

Pesquisa profunda (2 workflows, 8 agentes lendo o material do método + mercado) confirmou: **previsibilidade
é o núcleo do método** ("o futuro NUNCA pode ficar vazio — futuro vazio = projeção falsamente
otimista", o "chá revelação"). E a meta 20–30% é **média ANUAL** ("o ano tem que ser de 20 a 30;
tem mês que é mais, tem mês que é menos").

Mudanças:

- **Guardrail de poupança virou ANUAL sobre o REALIZADO** (`safe_to_spend_today` recebe
  `annual_income`/`annual_savings`; `realized_annual_savings`). O ano PROJETADO mente quando os
  meses futuros estão incompletos — o realizado pode ficar bem abaixo do projetado.
- **Cobertura por mês futuro** (`forecast::month_coverage` + `realized_monthly_baseline`): saída
  lançada ÷ mediana das saídas realizadas. Mês < 60% do típico = INCOMPLETO. Expõe o "buraco" de
  lançamento: meses futuros podem ficar bem abaixo do típico (faltam fatura + variáveis a lançar).
  Nenhum app líder faz isso — diferencial confirmado pela pesquisa de mercado.
- **UI**: card "Previsibilidade" (confiável até X, barras de cobertura, total faltante,
  orientação de pré-lançamento), poupança do ano realizada vs projetada, mensagem do guardrail
  anual. DTO: `annual_savings`, `coverage[]`, `trusted_through_month`, `total_missing_cents`.

**O que o método manda pré-lançar**: saldo inicial
(só conta-corrente), salário futuro (conservador; autônomo: só despesas), contas fixas
replicadas mês a mês, fatura do cartão no vencimento com parcelados, e o **diário estimado**
(soma das variáveis ÷ 30, em todos os dias — "nunca baixar na esperança"; deixar vazio é o erro
nº 1). Onboarding guiado + sugestão automática de diário = slices futuros.

### Slice 12 — Auditoria contra a planilha OFICIAL + fidelidade (2026-06-13)

Falha de método das revisões anteriores: comparavam a UI/banco contra **o próprio banco** —
circular (se o import errou, valida o erro). Correção: dois agentes adversariais compararam
contra a **planilha oficial** (um snapshot da planilha + notas de
célula). Achados e correções:

- **Descrições perdidas (P0)**: o import gravava `"Entrada/Saída {ano}"` e DESCARTAVA as
  **centenas de notas de célula** (a descrição real do método, no formato `"R$ X - Pagamento Conta A"`,
  `"Fatura serviço"`, `"Conta A/categoria"`). **Corrigido**: `SheetsClient::get_sheet_notes` (via `spreadsheets.get` +
  `includeGridData`) + `parse_rows_with_layout` usa a nota como descrição (quebras → " · "),
  fallback `"{kind} {date}"`. Caminho xlsx não tem notas (calamine) → fallback. **Re-importar**
  para popular.
- **Falso pânico "pode gastar R$ 0" (P1)**: a poupança realizada incluía o mês corrente em
  andamento (contas fixas já dentro, salário do mês ainda fora) → net negativo de timing.
  **Corrigido**: `realized_annual_savings` conta só **meses completos** (`substr(date) < mês`).
- **Performance otimista de meses incompletos (P1)**: o card mostrava meses futuros esparsos com
  taxas otimistas. **Corrigido**: a faixa marca meses incompletos (tracejado + "incompleto ⚠"), sem
  taxa enganosa.
- **Preview/detecção truncados (P1)**: `fetch_sheet_preview` (A1:Z21) e `detect_sheet_layout`
  (A1:Z10) cortavam colunas/linhas. **Corrigido**: grade inteira.
- **Valores/datas/Saldo EXATOS (confirmado)**: sem erro de 100×/locale; datas e série de Saldo
  batem com a planilha oficial célula a célula.

**Aba Economia**: a coluna `Economia` pode estar zerada em todos os meses para quem não lança
economia deliberada; nesse caso, pela métrica do método (Economia ÷ Entradas) a taxa de poupança
fica em 0% e a poupança real precisa ser lida pela Performance.

**Deferido (documentado)**: importar a aba Economia (slice 7); parsear as notas em **splits por
titular** (Conta A/B/C) e itens (a nota tem "R$ X - <item>" por linha); tile Diário oculto
quando zero; sparkline do Saldo anual.

### Slice 13 — Verificação final + coaching de adaptação (2026-06-13)

Verificação adversarial (4 lentes: app×método, robustez a edições, gaps/coaching, mercado SOTA).
**Veredito: ~70% fiel ao método.** Aplicado nesta rodada:

- **Staleness do `is_projection` (P1, o ponto do dono sobre dias passados/mês vigente)**: o flag é
  congelado no import; dias que viram passado (ou edições no mês corrente) ficavam stale.
  **Corrigido**: `realized_annual_savings`, `realized_monthly_baseline` e o `transaction_count`
  agora derivam "realizado" da **DATA** (mês completo / `date <= today`), não do flag. Teste
  `realized_annual_ignores_stale_is_projection_flag`.
- **Coaching do "colchão" (gap de adaptação do dono)**: ele não registra Economia formal (linha do
  método = R$ 0); guarda o excedente como **colchão** para cobrir meses negativos sem sacar
  investimento — adaptação VÁLIDA. **Card "Adaptação ao método — seu colchão"**: mostra "Economia
  registrada: R$ 0" × "Colchão realizado: R$ X (Y%)", reconhece a estratégia e ensina o próximo
  passo (formalizar Economia + reserva) sem punir. Princípio do mercado SOTA: "reconhecer antes de
  ensinar"; tom calmo; fases nomeadas (Mapear/Calibrar/Operar).

**Roadmap dos ~30% restantes (priorizado, do app×método):**

1. **Aba Economia** como fonte de verdade da poupança (substitui o proxy net) — slice 7.
2. **Entidade `invoice`/fatura** de 1ª classe (status/vencimento/fechamento/split-titular) — o
   diferencial crédito-first; hoje o crédito vem só de `daily_checkin`. Velocímetro de fatura.
3. **`reserve_months` dinâmico** = `reserve_cents / baseline_outflow_cents` (custo de vida × N);
   gate de compra grande da Mia depende disso.
4. **Saldos encadeados de meses futuros** na UI (o `month_end[]` já existe no DTO).
5. **Tags / owner / reembolso / pass-through** no schema (repasse de terceiro, fatura de Conta B = net-zero).
6. **Diário/velocímetro de crédito**, renda variável tipificada (extras/13º/PLR), ciclo de
   fechamento do cartão.

### Decisões de metodologia (estado após pesquisa de 2026-06-13)

- ~~**25% mensal vs 20–30% anual**~~ **RESOLVIDO**: confirmado que é média ANUAL; o guardrail agora
  usa a poupança anual sobre o REALIZADO (slice 11). 25% mantido como alvo (piso com folga de 5%).
- **Performance ≠ poupança declarada** (parcial): a poupança hoje é proxy = net realizado
  (renda − saída), rotulada honestamente como "poupança do ano realizada". O FIEL é a linha
  **Economia** da aba Economia (Economia ÷ Entradas) — depende do **slice 7** (importar a aba).
  Até lá, é proxy. Confirmado: performance e Economia são distintas (performance = entra − sai −
  diário − economia − cartões; fica negativa de propósito quando a economia é lançada).
- **Piso de reserva = 12 meses**: `reserve_floor` lê Bolsos `liquidity='reserve'` (0 sem config) —
  NÃO subtrai 12× automaticamente (o método não tem trava automática; é gate manual de compra
  grande). Custo de vida = Saída total − investimento/economia. Falta: modelar a reserva como
  meta + o gate "posso comprar?" (slice futuro / tool da Mia).
- **Tiles de empty-state honestos** (PENDENTE): "Crédito R$ 0", "Diário R$ 0", "Reserva 0,0m"
  leem tabelas vazias. Padrão 2026 (NN/g): mostrar "Configurar"/empty-state, nunca R$ 0 nem "—".
  A fazer numa passada de UI dedicada.

## Insumos do dono (necessários para o slice 2)

1. ~~APIs habilitadas~~ **Feito via CLI em 2026-06-12** (`sheets.googleapis.com` +
   `drive.googleapis.com` no projeto `neko-finance`).
2. **OAuth client "Desktop app"** criado no Console (sem caminho por CLI): branding/test
   user em `console.cloud.google.com/auth/branding` e `auth/audience`, client em
   `auth/clients/create` → fornecer **Client ID** (e o Secret, para o `.env`).
3. Criar `.env` local (gitignored) com `VITE_GOOGLE_CLIENT_ID=` e
   `VITE_GOOGLE_CLIENT_SECRET=`.
4. **URL (ou ID) da planilha real** para o primeiro dogfooding (ou colar direto no campo
   novo da UI).
5. Em reconexões futuras após mudança de scope, **reconectar** (novo consentimento).

## TDD

Regressão por bug (1, 3, 4, 8 da tabela); `parse_number` com a bateria de formatos reais;
parse com geometria real (JANEIRO offset 0, 12 blocos, célula espúria); idempotência do
re-import (idêntico/alterado/removido/aba isolada); reconciliação com fixture do mês real
(tolerância e arredondamento 4→2); horizonte multi-mês com `today` injetado.

## Limitações conhecidas (decisões conscientes, do review adversarial dos slices 0–1)

- **Aba esvaziada não limpa o banco**: se o parse de um re-import retorna zero linhas, o
  import é no-op (não deleta o que veio antes). Deletar tudo porque um parse retornou vazio
  (possível bug transitório) seria pior que dado obsoleto. Esvaziar uma aba-ano inteira não é
  cenário real de uso.
- **Mutação manual no banco + checksum idêntico**: se uma transação importada for apagada à
  mão no SQLite, o re-import da planilha inalterada é no-op e não a restaura (o checksum do
  batch ainda bate). Não há UI que delete transações hoje; revisitar quando houver.
- **Coluna `Data` como data-Excel**: se um xlsx de terceiros tiver a coluna de dia como data
  formatada (calamine `DateTime`), as linhas são puladas. A planilha real usa números 1–31.
  Primeiro suspeito se um dogfooding mostrar zero linhas importadas.
- **Preview mostra valores crus**: com `UNFORMATTED_VALUE`, a tela de preview exibe
  `1234.5600` em vez de `R$ 1.234,56`. Cosmético; o dado importado é o correto.

## Fora de escopo

Tudo da spec 008 (Pluggy/OFX, classificação de extrato, módulo Crédito, write-back,
`source`/`provider_txn_id`); leitura de notas de célula;
`payment_method`/`is_fixed` no import (Totais ficam imprecisos de propósito — documentado);
wire de `reserve`/`reserve_snapshot`; Mia.
