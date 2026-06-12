# Spec 008 — Import automático (Open Finance + OFX) + módulo Crédito

> Status: **validado com o usuário em 2026-06-12** (instituições, gates, rollback, toggle,
> Crédito dedicado, Economia, Pix→Diário). **Revisão 2 (2026-06-12):** correções do review
> multi-agente aplicadas — reconciliação de projeções futuras (EC14), matching de reembolso
> real↔previsto (EC15), pré-filtro por tipo de conta, geometria real da aba Economia,
> mecanismo novo de checksum por célula, fatos de mercado atualizados. Detalhes pessoais
> vivem no anexo privado gitignored `.methodology-pack/008-personal-mapping.md`.

## Problema

Hoje cada transação entra no Neko via import da planilha — digitada à mão. O objetivo é
**previsibilidade total com o mínimo de esforço manual**: o app puxa as transações sozinho,
classifica segundo o método e, ao escrever de volta na planilha, escreve **exatamente no
padrão que o dono já usa** (lump por coluna + nota estruturada na célula).

## Princípios (herdados do método / AGENTS)

1. **SQLite é o system-of-record**; a planilha é a visão canônica do método.
2. O método separa **Entrada / Saída (fixas+faturas) / Diário (variável)**; projeção encadeada
   dia a dia. O import automático NUNCA quebra essa semântica.
3. Compra no crédito **não toca o Diário**: acumula na fatura (`invoice`) e vira **Saída lump
   no vencimento** ("o cartão sequestra o salário futuro" — a fatura futura já está lançada).
4. **Nenhuma escrita na planilha sem confirmação humana** — ver §Write-back.
5. Classificação **determinística** (regras + correções do usuário); LLM não faz conta.
6. Vale alimentação/refeição **fica fora da planilha** (orientação do método); no Neko é só o
   bolso `restricted` (spec 007).
7. **Dinheiro em centavos inteiros** no domínio (padrão da spec 003). A planilha carrega
   floats com até 4 casas decimais; toda leitura arredonda a centavos na fronteira e toda
   comparação de reconciliação usa **tolerância explícita** (ver EC1) — nunca igualdade de
   float.

## Fontes de dados

| Rota                                                                                 | Para quê                                                   | Trade-off                                                                                  |
| ------------------------------------------------------------------------------------ | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| **A. Open Finance via Meu Pluggy** (conta PF gratuita + conta dev linkada via OAuth) | Sync de contas, cartões, transações e faturas              | Melhor cobertura; depende de terceiro; limites de conta dev (ver §Rota A)                  |
| **B. OFX/CSV local** (extrato/fatura do internet banking)                            | Fallback offline-first **onde a instituição exporta**      | Esforço manual mensal (~min), parser próprio; cobertura varia por instituição/produto      |
| Benefício (vale)                                                                     | **Sem conector** (benefício fora do Open Finance regulado; agregadores não cobrem operadoras de benefício — verificado 2026-06) | Saldo manual no bolso restrito |

A como caminho feliz, B como fallback, **mesmo pipeline** (a fonte é um adapter).

**Rota B não é universal**: a disponibilidade de OFX varia por instituição e por produto
(conta vs cartão) — algumas instituições exportam apenas CSV/PDF, outras só enviam OFX de
fatura **fechada** (a fatura aberta fica cega até a rota A cobrir). Por isso a rota B inclui
**parser CSV** além do OFX, e o mapa real instituição→formato disponível vive no anexo
privado. Onde nenhum export existe, a rota A é a única automática (ou entrada manual).

**Rota A — modelo de autenticação e limites**: as credenciais de desenvolvedor do agregador
(client id/secret) são operadas **exclusivamente no shell Rust** (o processo confiável do
app) e armazenadas no keyring do SO; o frontend só vê tokens de curta duração. Esse arranjo
diverge do modelo servidor-side recomendado pelo agregador — é aceitável para app pessoal
local-first (mesmo modelo do Actual Budget self-hosted), mas fica registrado como decisão
consciente. **Risco operacional documentado**: a conta dev gratuita tem período de trial;
após o trial, a lista de conectores pode não ser editável (adicionar instituição NOVA pode
exigir novo trial ou plano pago — modelo B2B). Conexões existentes seguem legíveis. Validar
os limites vigentes no slice 3, antes de prometer a rota A como permanente.

## Frequência de sync (decisão, baseada no padrão de mercado 2026)

Contexto: o agregador faz auto-sync em janelas de 24/12/8 h e os conectores Open Finance
regulados têm defasagem de **até 24 h**; webhooks (prática recomendada server-side) exigem
endpoint HTTPS público — inviável em app local-first sem servidor remoto. Portanto:

- **Automático: ao abrir o app**, com debounce (não re-sincroniza se o último sync tem < 8 h);
  re-sync oportunista se o app fica aberto > 8 h.
- **Manual: botão "Sincronizar agora"** sempre disponível (global e por conexão), ignora o
  debounce.
- **Retroatividade do primeiro sync: 1º de janeiro do ano vigente** (alinha com a aba-ano da
  planilha). Anos anteriores permanecem como histórico **lump-only** (importados da planilha,
  nunca reconciliados com banco); métricas que misturam períodos (médias, série de Economia)
  devem rotular a base de cada período.

**Controle do usuário:** o auto-import tem **toggle global e por conexão** (pausar/desativar/
remover conexão + revogar consentimento). Desativado = o app segue 100 % funcional com
import manual/planilha.

**Remover conexão não apaga dados**: as transações já importadas permanecem no SQLite com a
`connection` marcada como removida (órfãs rastreáveis); reconectar a mesma instituição
reimporta pelo dedup normal (provider_txn_id + fingerprint), sem duplicar. Ciclo de vida
completo: `enabled → paused → revoked → removed`, sempre sem perda de histórico.

## Pipeline

```
[Pluggy API | arquivo OFX/CSV]
  → normalizar (RawBankTxn: provider_id, conta de origem, data, valor, descrição, tipo)
  → dedup (provider_txn_id + fingerprint; re-sync idempotente; cruza fontes A×B)
  → classificar (motor de regras determinístico)
  → persistir (transaction/split/invoice no SQLite)   ← até aqui SEM tocar a planilha
  → recalcular forecast
  → propor write-back → preview + aprovação → Sheets  ← único ponto que escreve fora
```

## Classificação (regras em ordem; primeira que casa vence; auditável)

> **Pré-filtro obrigatório (antes de qualquer regra):** o motor resolve o `account.type` da
> conta de origem do `RawBankTxn`. **Itens originados em conta `credit_card` vão direto à
> regra 8** (item de fatura; nunca passam pelo catch-all da regra 7) — sem isso, no perfil
> crédito-first, todo gasto de cartão viraria Diário E lump no vencimento (dupla contagem).
> As regras 1–7 aplicam-se a contas de débito/corrente. A ordem importa também dentro do
> grupo: pagamento de fatura, transferências internas e reembolsos previstos são
> débitos/créditos em conta e seriam classificados errado pelas regras genéricas se
> avaliados depois.

1. **Pagamento de fatura** (débito em conta que casa com fatura fechada/aberta de cartão
   conectado) → `Saída` (bloco CARTÕES) e **reconcilia a `invoice`** (paid/partially_paid).
   **Precedência dura sobre o detector de transferências**: o par débito-em-conta ↔ crédito
   na conta-cartão do mesmo emissor é sempre pagamento de fatura (regra 1), nunca `transfer`
   neutro da regra 2/EC4.
2. **Transferência entre contas próprias da MESMA classe de liquidez** → `transfer` interno,
   neutro (não é Entrada nem Saída).
3. **Transferência entre classes de liquidez segue a regra usar↔repor do método**:
   - `reserve → liquid` = **`Entrada` "uso de reserva"** na planilha + o app **sugere a
     `Saída` futura de reposição** (obrigatória no método);
   - `liquid → reserve` = **aporte de economia** (alimenta a aba `Economia` do mês), não é
     Saída de consumo;
   - movimentos envolvendo o bolso `restricted` (vale) não tocam a planilha (fora do método),
     só o ledger do bolso. **Atenção**: aportes que o dono LANÇA na planilha como
     `Investimento:` dentro de `Saída` (ex.: previdência) **fazem parte da planilha e do
     método** (custo de vida = Saída − investimentos, §5 do método) — `illiquid` não
     significa "invisível"; significa apenas que o saldo do bolso não entra no caixa
     projetado (spec 007).
4. **Crédito que casa com uma `Entrada` de reembolso PREVISTA** (contraparte conhecida,
   valor total **ou parcial**, janela ampla — semanas, ver EC15) → **abate o saldo devedor
   da contraparte** (EC12) e substitui/realiza a Entrada prevista correspondente.
   **Condição de disparo (dupla, obrigatória)**: (a) existe `counterparty_balance` com
   residual > 0 dentro da janela E (b) a descrição normalizada do crédito casa com o
   matcher da contraparte (`classification_rule` de **tipo reembolso** — distinto dos
   matchers de renda). Sem os DOIS sinais, o crédito cai na regra 5 — um salário nunca é
   casado contra um saldo devedor só por coincidência de valor. **Nunca
   cria Entrada nova em paralelo à prevista** — sem esta regra, a parte da contraparte
   contaria duas vezes (a prevista net-zero + o crédito real da regra 5).
5. Crédito em conta corrente → `Entrada` (salário, rendimentos, reembolsos avulsos).
   Rendimentos diários da mesma origem **agregados por dia** — os itens individuais são
   persistidos e deduplicados um a um (o dedup roda ANTES da classificação); o agregado
   diário é **derivado/recalculado idempotentemente** (nunca uma transação persistida com
   soma congelada), para que um rendimento que chega atrasado (fim de semana → segunda)
   atualize o agregado em vez de duplicá-lo.
6. Débito que casa com despesa fixa conhecida → `Saída` (bloco CONTAS). **Âncora no schema**:
   "despesa fixa conhecida" = `category.nature='fixed'` (existente, semeada na migração 005)
   combinada com matcher de descrição/instituição.
7. **Demais débitos e Pix avulsos → `Diário`** (decisão do dono: sem limiar de revisão).
   O método confirma: **Diário reflete só débito/dinheiro**; crédito nunca entra aqui.
   **Vigência: do primeiro sync em diante (forward-only)** — o histórico da planilha lança
   variável avulso em `Saída` e o backfill NÃO reclassifica o passado (ver EC13).
8. Transação de cartão de crédito → item da `invoice` aberta (NUNCA Diário); itens do
   **cartão adicional** (detectado por `account.linked_account_id` apontando para o cartão
   principal; `owner_person_id` da conta adicional identifica a contraparte) →
   `split.owner_person_id` do titular → **Entrada de reembolso prevista** no vencimento
   (net-zero), linkada via `split.reimbursed_by_transaction_id`. Encargos financeiros do
   cartão (juros de rotativo, multa, IOF de ciclo) têm **matcher builtin por padrão de
   descrição** → item da invoice do ciclo em que caem, marcados `encargo_financeiro` (nunca
   caem no catch-all).
9. Correções do usuário viram `classification_rule` (matcher instituição+descrição→destino),
   aplicadas antes das genéricas — categorização que "aprende" sem ML. **Retroatividade
   controlada**: uma regra nova NÃO reclassifica transações cuja classificação já foi
   escrita na planilha em lote aprovado (`classification_locked`); se a reclassificação
   afetaria células aprovadas, o app gera uma **nova proposta de write-back** (gate normal)
   com o diff das células afetadas e registra a transição de coluna na trilha do
   `sync_batch` — nunca muda silenciosamente.
10. **Tags do método** (`reembolso`, `dividir com alguém`, `pago-por-terceiro`…): qualquer
    `Entrada` pode ser marcada como reembolso **linkado** a uma Saída/parcela (contrapartes
    recorrentes além do cartão adicional: empréstimos a terceiros, rachas, pagamento por
    terceiro, divisão de contas da casa — padrão comprovado no uso real da planilha).

### Integração com as Réguas (dual-tracking da spec 001)

O pipeline alimenta o `daily_checkin`: débitos classificados como Diário somam em
`daily_spend` (Régua 1) e itens de cartão do dia somam em `credit_spend` (Régua 2) — o
velocímetro diário passa a se preencher sozinho. **Mecânica**: o pipeline **recalcula** o
agregado do dia por pessoa (upsert determinístico, substitui — nunca incrementa, para ser
idempotente); um check-in lançado manualmente pelo dono prevalece sobre o calculado e o
conflito vai para a caixa de revisão.

## Módulo Crédito (visão dedicada — requisito central)

A planilha só registra o lump; o Neko mostra o que a nota da célula não consegue:

- **Faturas**: todas as faturas (abertas/fechadas/pagas) por cartão, ciclo
  (fechamento/vencimento), total acumulando dia a dia ("velocímetro de crédito").
- **Separação por titular (o mais importante)**: cada item da fatura atribuído ao dono do
  cartão (titular vs adicional); visão "minha parte / parte do(a) parceiro(a)" por fatura,
  com o reembolso previsto linkado e o **saldo devedor por contraparte** (EC12/EC15).
- **Parcelados**: agenda completa (`n/total`, valor, cartão, término), total comprometido por
  mês futuro — o "salário futuro sequestrado" visível.
- **Limites**: limite, usado e disponível por cartão (dados do agregador).
- **Simulador de compra**: à vista vs parcelado em N× → impacto no forecast encadeado,
  com o **gate determinístico do método**: (1) a reserva cai abaixo de **12 meses** de
  cobertura (default do método; configurável — pisos: 6 mínimo, 6–8 amarelo, 12+ verde;
  custo de vida = Saída total − investimentos/economia, **sem descontar** "vou gastar
  menos")? (2) a nova parcela impede economizar 20–30 %? Ambos "não" → pode; fluxo futuro
  negativo → não. Só bem durável. O simulador também aplica as **heurísticas qualitativas
  do método**: preferir prazo longo/parcela menor ("não contar que sempre pago parcela
  alta"), entrada ~10 % quando parcelar, e — quando há data-alvo de viagem — alertar que o
  parcelamento deve **terminar ≥ 1 mês antes** da viagem. Simulação é cenário, não escreve
  em lugar nenhum.

## Economia (aba `Economia` — ativar o pilar não usado)

**Geometria real da aba** (uma única aba, blocos-ano lado a lado): cada bloco-ano tem as
colunas `mês | Entradas | Economia | %`; `Entradas` de cada mês é **fórmula com referência
própria por mês** (stride de 6 colunas na aba-ano: `'<ano>'!B38`, `H38`, `N38`, …); `%` é
majoritariamente fórmula `Economia/Entradas`, mas **células históricas podem ser literais**.
O write-back resolve **ano→bloco e mês→linha dinamicamente** — nunca assume coluna fixa.
**Algoritmo de detecção**: varrer a linha de cabeçalho da aba procurando células cujo valor
é o NÚMERO do ano; cada bloco-ano = a célula do ano + as três colunas seguintes
(`Entradas | Economia | %`), com os meses nas linhas abaixo em ordem (jan..dez + TOTAL).
`Economia` é sempre a **2ª coluna após a célula do ano** — é essa que o write-back localiza
antes de qualquer escrita. (O cabeçalho aqui é numérico — detecção própria, não reutiliza o
matcher de nomes de mês do `detect_sheet_layout` da aba-ano.)

Semântica (fiel ao método):

- **Economia = aporte deliberado**, não resíduo: o valor escrito vem de transferência
  `liquid → reserve` (regra 3) ou de `Investimento:` lançado — exatamente o que o dono
  digita à mão hoje. A **sobra real do fluxo é apenas o TETO sugerido** na UI ("você pode
  economizar até X"), nunca gravada automaticamente. Aportes já lançados como
  `Investimento:` em `Saída` **não contam duas vezes** (custo de vida = Saída −
  investimentos).
- `%` = Economia/Entradas. **O farol avalia a média anual acumulada** (meta = 25 % de piso
  com folga; a faixa "20–30 %" é **média anual**, não gate mensal) — o % mensal é exibido
  informativo/neutro, sem pintar de vermelho meses estruturalmente fora da faixa.
- Tela Totais/Reserva mostra a série mensal e a régua; deriva também **Custo de vida**
  (definição canônica do método: **Saída total − investimentos/economia** — a mesma do gate
  do simulador; "fixas + diário + cartão" é a expansão equivalente quando o investimento
  está lançado em Saída) e **Diário médio real** (gasto real / dias — "a estrela da
  casa"), os dois indicadores da aba Totais do método. Uso de reserva segue a regra
  **usar↔repor** do método (uso = Entrada; repor = Saída futura obrigatória).
- Write-back: **apenas a célula `Economia` do bloco-ano correto**, e **apenas para meses
  com `Entradas` ≠ 0** (escrever antes da Entrada existir exporia divisão por zero na
  fórmula `%`). `Entradas` e `%` nunca são escritas — `%` inclusive tem células literais
  históricas que não devem ser "recalculadas". Passa pelo mesmo gate de aprovação.

## Write-back: preview, confirmação e rollback (invariantes)

- **Jamais** escreve na planilha sem confirmação explícita. Não existe modo "auto-aprovar".
- **Preview total** (ApprovalDiffCard): célula a célula — valor antigo → novo, nota antiga →
  nova, com diff textual da nota; agrupado por dia/coluna; aprovar tudo ou por item.
  O diff de valor compara o **valor numérico** (centavos), não a string da fórmula — mudança
  puramente cosmética de forma não é proposta.
- **Rollback claro**: cada lote aprovado vira um `sync_batch` com snapshot before/after de
  todas as células tocadas; a UI lista os lotes aplicados com **"Desfazer"** (restaura valores
  e notas anteriores, também via gate de confirmação). **O rollback verifica o checksum atual
  da célula contra o snapshot `after` do lote**: se o dono editou a célula depois do apply,
  o app avisa ("célula editada após o lote; desfazer sobrescreve sua edição") antes de
  prosseguir — nunca desfaz silenciosamente por cima de edição manual.
- **Conflito de célula**: o `sync_log_checksum` **existente cobre apenas dedup de batch do
  import da planilha** (checksum do conjunto de linhas importadas) — ele NÃO protege células.
  O write-back exige um mecanismo **novo**: `sync_batch.cell_checksum` (hash de valor+nota de
  cada célula capturado no momento do preview), verificado imediatamente antes do write; se a
  célula mudou, o lote não aplica e o diff é re-gerado (EC11).

### Formato escrito (gramática validada contra as notas reais — anexo privado)

A gramática das notas **varia por ano** (o padrão do dono evoluiu); o renderer **detecta a
gramática da aba/célula, não impõe um template**:

- `Saída` = lump; nota com blocos de cabeçalho em linha própria. Cabeçalhos observados (o
  conjunto completo, com variantes de caixa e pontuação, está no anexo privado): `CONTAS` /
  `Contas:`, `CARTÕES` / `Fatura:` / `Faturas:` / `FATURAS:`, `Investimento:`, `OUTROS`,
  `AJUSTES`. O matching de bloco é **case-insensitive com sinônimos**; há células com itens
  **sem cabeçalho algum** — nesse caso o merge apensa no estilo da célula (item direto),
  **nunca inventa um cabeçalho** que o dono não usou. Atenção: o dono usa `CONTAS` também
  para faturas recorrentes de serviços (telefonia) — a heurística "instituição → CARTÕES"
  não vale; o bloco-destino segue a regra aprendida/correção, não o tipo do payee.
- `Entrada` = lump; nas abas mais antigas a nota usa cabeçalho `Entradas:`; **no ano vigente
  as notas de Entrada não têm cabeçalho** (itens diretos `R$ X - Descrição (data)`).
  Rendimentos como `Rendimentos <Instituição>`.
- `Diário` = soma do dia **(apenas o valor; o detalhe dos itens fica só no SQLite)**; a nota
  da célula Diário carrega o orçamento mensal do dono e **não é tocada jamais**.
- Parcelados mantêm sufixo `n/total` na descrição.
- **Formato canônico de itens NOVOS**: `R$ 1.234,56 - Descrição` (vírgula decimal, ponto de
  milhar). Itens **existentes nunca são reformatados** — o merge só apensa.

### Invariantes de célula (a planilha é uma engine, não uma tabela)

- **Preservar a forma da célula, não impor `=SUM`**: as células reais são uma mistura de
  número literal puro e fórmulas `=SUM(v1+v2+…)` — frequentemente com **quebras de linha
  internas** que o dono digita. Regra: ao apensar a uma célula existente, manter a forma
  atual (literal de termo único permanece literal; apensar a um literal converte para
  `=SUM(antigo+novo)`; apensar a um `=SUM` insere o termo preservando as quebras de linha
  existentes). Célula nova: literal para 1 termo, `=SUM` para vários — espelhando o padrão
  do dono. _(resolve a decisão em aberto do plano §7: a célula continua editável à mão no
  formato que o dono realmente usa)_
- **NUNCA tocar as colunas `Data` e `Saldo`** — `Saldo` é a fórmula encadeada da
  previsibilidade (`prev + Entrada − (Saída + Diário)`); sobrescrever quebra a engine.
- **Mapeamento data→célula valida os dias reais do mês**: a geometria tem linhas fixas para
  dias 1–31 em todos os meses (fevereiro tem linhas de dia 29–31 com fórmulas herdadas);
  o write-back nunca escreve em linha de dia inexistente e a reconciliação ignora essas
  linhas.
- Na aba `Economia`, escrever **apenas a coluna `Economia` do bloco-ano correto** (ver
  §Economia — geometria com dois blocos lado a lado e `Entradas` com stride por mês).
- **Merge de nota, nunca replace**: itens novos são apensados ao bloco correto da nota
  existente, preservando 100 % do texto manual do dono; o diff do preview mostra a nota
  final inteira. **Idempotência do merge**: antes de apensar, o item é procurado na nota por
  (valor, descrição normalizada — tolerante a variações de espaçamento/caixa do texto do
  dono); aprovar → desfazer → re-aprovar não duplica itens. O renderer respeita o limite de
  tamanho de nota do Sheets (células de fatura com muitos itens): ao se aproximar do limite,
  trunca com sumário explícito no preview — nunca silenciosamente.

## Edge cases & reconciliação (padrão de mercado aplicado)

Estado da arte (Actual Budget/YNAB): **(1)** id do provedor como 1ª defesa de dedup;
**(2)** sem id, fuzzy match por valor idêntico + janela de dias + payee similar; **(3)** em
colisão manual×importado, **a importada vence** e a manual é _linkada/promovida_, nunca
duplicada; **(4)** ambíguos vão para fila de revisão, não são resolvidos no chute.

### EC1 — Planilha atualizada à mão com conector defasado (caso crítico)

O dono lança o lump manualmente; horas depois o sync traz as mesmas transações do banco.
A célula é um **agregado** (`=SUM(...)`), então a reconciliação é em **nível de
dia × coluna**, não item a item:

1. Import da planilha marca transações com `source='sheet'` (lump do dia por coluna).
2. No sync, o pipeline classifica os itens bancários e computa a soma esperada por
   dia × coluna.
3. **Soma bancária ≈ lump da planilha** (tolerância: |Δ| ≤ max(R$ 0,02; R$ 0,01 × nº de
   itens bancários do dia×coluna) — a planilha carrega floats de 4 casas e resíduos de
   arredondamento que crescem com o número de termos; comparação sempre em centavos, nunca
   `==` de float) → itens bancários são gravados como o detalhamento e o lump vira
   `reconciled_by` deles (forecast conta UMA vez; o lump deixa de pontuar). **A célula NÃO
   é reescrita** — o lump manual é preservado na planilha; o detalhamento vive só no SQLite.
   O casamento tenta o dia exato e depois **janela de ±2 dias** (o dono lança no dia da
   compra; o banco posta D+1/fim de semana — sem janela, todo lump manual divergiria).
4. **Somas divergem além da tolerância** → fila de revisão com diff (itens bancários vs
   lump), o dono decide: aceitar detalhamento (write-back propõe corrigir a célula — **na
   linha/dia que o dono usou**, ver EC8), manter lump (itens ficam `shadowed`), ou casar
   parcialmente (modelado por `reconciliation_link`, ver §Modelo de dados). Decisões são
   persistidas: o re-sync **não re-propõe** o que o dono já decidiu.
5. Lançamento manual unitário (não-lump) casa por **valor exato + janela de ±5 dias +
   descrição normalizada**; match → merge mantendo o id do provedor.

**A janela opera sobre a linha do tempo em datas absolutas** — o mapeamento
data→(aba do ano, bloco de colunas do mês, linha do dia) acontece **só na hora de ler/
escrever a célula**. Consequência obrigatória: a janela de ±2/±5 dias **atravessa fronteira
de mês** (lump em 31/jan ↔ itens postados em 01–02/fev, que vivem em outro bloco de
colunas) **e de ano** (31/dez ↔ 01/jan, outra aba). Golden tests dedicados (§Testes).

Invariante: **uma realidade econômica = um efeito no forecast**, independentemente de
quantas fontes a reportaram.

### EC2 — Pendente → efetivada

Transação de cartão/Pix pode mudar de valor/data ao liquidar (e o provedor pode trocar o
id). Guardar `status` (pending/posted); na efetivação, casar por valor+janela e **atualizar
in-place**, nunca inserir segunda linha. **Antes de inserir um posted sem par, consultar o
tombstone (EC10) também pela identidade do pending** (provider_txn_id original), não só pelo
fingerprint — uma pending excluída pelo dono que efetiva com valor/id diferentes não pode
ressuscitar.

### EC3 — Ids do provedor instáveis

Reconexão/recriação do item no agregador pode renumerar `provider_txn_id`. Dedup tem
fallback de **fingerprint** (conta + data + valor + descrição normalizada + multiplicidade
no dia) — duas compras idênticas legítimas no mesmo dia não são sobre-deduplicadas porque a
multiplicidade conta. **Na rota B (OFX), o `provider_txn_id` é a chave composta
`instituição + conta + FITID`** (FITID só é único dentro da conta, e implementações de
bancos BR podem regenerá-lo entre exports — por isso o fingerprint é a defesa primária da
rota B, com FITID como sinal). **Dedup cross-fonte**: a mesma transação chegando pela rota A
E pela rota B é deduplicada pelo fingerprint; a versão da rota A tem precedência e
enriquece o registro (preserva o id mais estável), nunca duplica.

### EC4 — Transferência entre contas próprias

Banco principal→conta remunerada etc. NÃO é Entrada nem Saída (inflaria os dois lados). Detectar
par (mesmo valor, datas próximas, contas internas) → `transfer` interno, fora do
Entrada/Saída do método. **Exceção dura (= regra 1): se uma das pernas é conta
`credit_card`, o par é pagamento de fatura — reconcilia a `invoice` e vira `Saída` no bloco
CARTÕES**; o detector de transferências nunca o neutraliza.

### EC5 — Estorno, reembolso e compra internacional

Estorno no cartão = item negativo na fatura do ciclo em que cai (reduz o lump futuro).
Compra internacional: IOF/ajuste cambial chegam como itens separados — ficam na fatura como
itens próprios (sem tentar fundir).

### EC6 — Pagamento parcial da fatura / rotativo

Pagamento ≠ total da fatura fechada → invoice fica `partially_paid` com saldo residual
(+ encargos no ciclo seguinte como itens, via matcher de encargos da regra 8). O match
pagamento↔invoice usa tolerância e cai em revisão se ambíguo. **Semântica do lump e do
residual**: o lump escrito na planilha reflete o valor **efetivamente debitado** (o pago) —
nunca o total da fatura; o residual + encargos são **projetados como Saída no vencimento do
ciclo seguinte** (data explícita no forecast) e o módulo Crédito mostra o saldo devedor da
fatura. Assim a célula que o dono vê bate com o débito real E a dívida remanescente fica
visível.

### EC7 — Parcelados

Parcelas mensais chegam com o mesmo descritor (`n/total` quando o emissor informa). Ligar à
`installment_plan` existente em vez de criar plano novo; antecipação de parcelas remove as
futuras correspondentes (com revisão).

### EC8 — Datas divergentes

Data da compra vs data de lançamento no extrato (D+1, fim de semana): a janela de match
absorve; a data canônica é a do extrato (consistência com o saldo bancário — mesma escolha
do Actual Budget). **A data canônica governa o ledger interno**; na planilha, ao reconciliar
um lump manual, o write-back **preserva a linha/dia que o dono escolheu** — nunca migra o
valor de linha (mover de dia deslocaria o Saldo encadeado intra-mês). Itens novos sem lump
manual são escritos no dia canônico do extrato.

### EC9 — Consentimento expirado / conexão quebrada

O consentimento Open Finance expira **no prazo negociado com a instituição** (a Resolução
Conjunta BCB/CMN nº 7/2023 eliminou o teto fixo de 12 meses; consentimentos podem ser de
longa duração) — o app usa **a data real de expiração retornada pelo agregador**, nunca um
timer fixo. Consentimento expirado ou item quebrado → estado visível na tela Conexões
("desatualizado desde X") + aviso no Dashboard de que o forecast pode estar defasado.
Renovação exige ação explícita do dono. Nunca falhar em silêncio. (Política de dados ao
remover conexão: ver §Controle do usuário.)

### EC10 — Transação deletada pelo usuário reaparecendo no re-sync

Exclusões locais guardam o fingerprint **e o provider_txn_id** num "túmulo" (`tombstone`):
o re-sync não ressuscita o que o dono apagou (configurável, default ligado). **Nota de
mercado**: o default do Actual Budget é o OPOSTO (`reimportDeleted=true`, deletadas voltam
no re-import) — a escolha do Neko **diverge intencionalmente** para proteger exclusões do
dono. Interação com EC2: ver consulta ao tombstone por identidade do pending.

### EC11 — Célula editada durante a aprovação do write-back

Coberto pelo mecanismo **novo** `sync_batch.cell_checksum` (ver §Write-back): o lote só
aplica se cada célula ainda tem o checksum capturado no preview; senão, re-gera o diff.
(O `sync_log_checksum` existente é outra coisa — dedup de batch do import — e não protege
células.)

### EC12 — Reembolso parcial/atrasado da contraparte (comprovado no uso real)

A parte do cartão adicional (ou outra contraparte) pode ser paga **parcial** e/ou **fora do
vencimento** ("pagamento parcial" + "restante" em meses distintos acontece na planilha
real). A Entrada de reembolso prevista vira **saldo devedor por contraparte**
(`counterparty_balance`, ver §Modelo de dados): pagamentos parciais abatem; o residual
reprojeta para data futura (visível no módulo Crédito e no forecast). Reembolso no mês
errado muda a projeção — o timing é parte do método. O matching do crédito bancário real
contra a previsão é a regra 4 / EC15.

### EC13 — Primeiro sync (backfill do ano) × planilha já preenchida

O backfill desde 1º de janeiro encontra a planilha INTEIRA já lançada à mão → é o EC1 em
escala: a reconciliação roda mês a mês, dia × coluna, e divergências entram na fila de
revisão agrupadas por mês (não uma avalanche de itens soltos).

**Divergência estrutural Saída↔Diário (fato do histórico real)**: o dono historicamente
lança gasto variável avulso em `Saída` e a coluna `Diário` fica ≈ 0 — enquanto a regra 7
classifica Pix/débito avulso como Diário. Sem tratamento, TODO dia com gasto divergiria em
duas colunas ao mesmo tempo. Política: **no backfill, a reconciliação por coluna que falha
re-tenta sobre `Saída + Diário` combinados no dia** (o "Saída Total" da planilha); se a soma
combinada bate, reconcilia sem revisão e **sem propor mover valores entre colunas** — a
regra Pix→Diário vale do primeiro sync em diante (forward-only), nunca reescreve o
histórico.

**UX de onboarding**: a meta "< 1 min/dia" da caixa de revisão aplica-se ao **regime
contínuo**; o primeiro sync tem UX dedicada — aprovação em lote por mês e opção de pular
meses históricos (ficam lump-only, rotulados).

**Chá-revelação preservado**: o backfill é registro histórico (médias, Economia,
reconciliação) — a regra do método "começar HOJE, nunca retroativo" continua valendo para a
PROJEÇÃO: a âncora do forecast é o **saldo real reconciliado de hoje** (spec 003), nunca a
soma do histórico. O backfill NÃO realimenta a semente nem o "pode gastar até X".

### EC14 — Lançamentos FUTUROS pré-projetados na planilha × projeção do app

**Fato real**: o dono pré-lança meses futuros à mão (salário no fim de cada mês, contas
fixas recorrentes, aportes) — a planilha já carrega projeção manual do ano inteiro. O app
TAMBÉM gera projeções (fatura futura no vencimento, reembolso previsto, recorrências). Sem
reconciliação, **a mesma realidade futura pontua duas vezes** no saldo projetado (o herói
do método erraria por múltiplos salários/faturas até dezembro).

Regras:

1. Import da planilha marca lançamentos com data futura como **projeção do dono**
   (`source='sheet'` + `is_projection`), distintos de lumps passados (a reconciliar via
   EC1/EC13).
2. **Uma realidade futura = um efeito**: quando o app geraria uma projeção própria para a
   mesma realidade (mesma natureza, valor ≈, mesma janela de vencimento/dia), a projeção do
   app é **suprimida/linkada** (`reconciled_by`) à projeção do dono — a do dono vence (é a
   planilha canônica). Sem par manual, a projeção do app pontua normalmente.
3. Lumps futuros do dono são **intocáveis pelo write-back** até a data chegar; quando o
   banco confirma a transação real, aplica-se EC1 normalmente (o real reconcilia a projeção
   — dela herdando a célula).

### EC15 — Matching do reembolso REAL contra a Entrada prevista

Complemento da regra 4 + EC12 (a peculiaridade central do cartão adicional). O crédito
bancário real da contraparte chega **parcial, fracionado e em datas variadas** (semanas de
diferença do vencimento; às vezes a soma não fecha com a Saída por arredondamento do
acerto). Regras:

- Matcher: contraparte conhecida (descrição/regra aprendida) + valor ≤ saldo devedor +
  janela ampla (default: o mês do vencimento ± 1 mês). Match → abate `counterparty_balance`
  e realiza a Entrada prevista (parcial ou total); resíduo reprojeta (EC12).
- Ambíguo (dois saldos devedores compatíveis, valor não bate com nenhum) → fila de revisão,
  nunca chute.
- **Nunca** deixar coexistir a Entrada prevista viva + o crédito real classificado como
  Entrada genérica — é dupla contagem direta no forecast.

### Testes exigidos (além dos do §TDD)

Golden tests para CADA EC acima; EC1 com os três desfechos (igual, divergente, parcial) e
com lump em D±2 **incluindo travessia de fronteira de mês (31→01) e de ano (31/dez→01/jan,
muda a aba)**; EC1 com lump de 4 casas decimais (tolerância em centavos, nunca igualdade de
float); EC3 com multiplicidade 2 no mesmo dia e com FITID regenerado entre exports;
transferências (regras 2–3) nas quatro combinações de liquidez **+ pagamento de fatura
intra-banco que NÃO vira transfer (regra 1 > EC4)**; EC12/EC15 com pagamento parcial +
restante em mês seguinte casando contra o saldo devedor (sem Entrada duplicada); EC13 com
ano inteiro pré-lançado **+ o caso Saída↔Diário combinados**; EC14 com salário/fatura
pré-lançados em mês futuro (projeção do app suprimida; real chegando reconcilia); write-back
nunca emite range que toque `Data`/`Saldo`/`Entradas`/`%` da aba Economia nem linha de dia
inexistente do mês; merge de nota preserva byte a byte o texto manual, **é idempotente sob
aprovar→desfazer→re-aprovar** e respeita célula literal vs `=SUM` com quebras de linha;
classificação: item de conta `credit_card` NUNCA cai na regra 7 (pré-filtro); propriedade
global: re-rodar o sync N vezes é **idempotente** (estado final idêntico — com regras
aprendidas congeladas para transações `classification_locked`).

## Modelo de dados (estende, não recria)

> Campos/entidades **novos** exigem migração — nada abaixo existe hoje, exceto onde marcado.

- `connection` (instituição, rota A/B, status enabled/paused/revoked/removed, last_sync_at,
  consent_expires_at vindo do agregador) — token/credencial no keyring do SO, NUNCA no banco
  nem no repo.
- `invoice` (account_id, ciclo, closing/due date, status open/closed/paid/partially_paid,
  total, saldo residual) + `transaction.invoice_id`.
- `installment_plan` (compra parcelada: total, n_parcelas, cartão) + parcelas como
  transações futuras linkadas.
- `transaction.provider_txn_id` (**novo**; rota B usa chave composta instituição+conta+FITID)
  e `transaction.source` (**novo**: sheet/openfinance/ofx/manual) — exige **retrofit do
  importador de planilha existente** para marcar `source='sheet'` (hoje ele insere sem
  origem; ver §Sequência, passo 1).
- `classification_rule` (matcher, destino, **tipo**: renda/reembolso/encargo/genérica — a
  regra 4 só casa matchers de tipo reembolso; o matcher builtin de encargos da regra 8 é
  tipo encargo, e o item resultante leva a marca `encargo_financeiro` na invoice para o
  módulo Crédito separar encargo de consumo; origem builtin/user-correction) +
  `transaction.classification_locked` (classificação já escrita em lote aprovado).
- `sync_batch` (**novo** — snapshot before/after por célula + `cell_checksum` do preview,
  para gate de conflito e rollback; distinto do `sync_log` existente, que só deduplica
  batches de import).
- `transaction.status` (pending/posted), `.reconciled_by` (lump↔detalhamento e
  projeção-do-app↔projeção-do-dono, EC1/EC14), `.shadow_status`
  (null/shadowed/partially_reconciled, EC1) + `reconciliation_link` (lump_id, detail_id,
  matched_amount — casamento parcial), `transfer` interno (EC4), `tombstone` de exclusões
  (fingerprint + provider_txn_id, EC10), fingerprint de dedup (EC3).
- `transaction.is_projection` (**já existe** — migração 006, sem migração nova): o
  importador de planilha passa a setá-lo em lançamentos com data futura (projeção do dono,
  EC14).
- `split.owner_person_id` (já existe) + `split.reimbursed_by_transaction_id` (**novo** —
  link da parte da contraparte à Entrada de reembolso) + `counterparty_balance`
  (view/entidade: contraparte, total devido, pago, residual, data reprojetada — EC12/EC15).
  Detecção do cartão adicional via `account.linked_account_id` (já existe).
- `account.credit_limit`, `closing_day`, `due_day` (já existem desde a migração 003)
  populados pelo sync.

## UI

1. **Ajustes → Conexões**: conectar instituição (widget Pluggy) ou importar OFX/CSV; status,
   último sync, data real de expiração do consentimento, **toggle por conexão + toggle
   global**, revogar/remover (sem perda de histórico).
2. **Caixa de revisão**: transações novas com classificação proposta; corrigir ensina o
   motor. Meta < 1 min/dia **em regime contínuo** (onboarding do primeiro sync tem fluxo
   próprio por mês, ver EC13).
3. **Tela Crédito**: faturas, split por titular, saldo devedor por contraparte, parcelados,
   limites, simulador.
4. **ApprovalDiffCard + histórico de lotes com Desfazer**.

## TDD obrigatório

Normalização OFX/CSV/Pluggy; dedup idempotente (incluindo cross-fonte A×B e FITID composto);
cada regra de classificação + o pré-filtro por tipo de conta; agregação de rendimentos
(recalculada, item atrasado não duplica); ciclo de fatura (closing/due, virada de mês/ano);
reconciliação pagamento↔invoice (incluindo parcial: lump = pago, residual projetado);
reembolso do adicional (matching real↔previsto, EC15); agenda de parcelados; simulador
(puro, gate de 12 meses + heurísticas); Economia (aporte deliberado, farol anual, detecção
ano→bloco→linha pelo cabeçalho numérico, nunca escrever com Entradas=0); Custo de vida
(Saída total − investimentos/economia) e Diário médio real derivados idempotentemente; render da nota (golden tests contra a
gramática real POR ANO, incluindo células sem cabeçalho e literal vs `=SUM` com quebras);
snapshot/rollback de `sync_batch` (incluindo rollback bloqueado por edição posterior);
conflito por `cell_checksum`; **migrações das tabelas/colunas novas** (connection, invoice,
installment_plan, sync_batch, transaction.source/provider_txn_id/status/…,
reconciliation_link, counterparty_balance); upsert do `daily_checkin` (recalcula, manual
prevalece).

## Sequência de implementação (vertical slices)

1. **Migrações base** (TODAS as colunas/tabelas novas do §Modelo de dados:
   transaction.source/provider_txn_id/status/shadow_status/classification_locked, invoice,
   installment_plan, reconciliation_link, counterparty_balance,
   split.reimbursed_by_transaction_id, sync_batch, connection, tombstone) + **retrofit do
   importador de planilha** (marcar `source='sheet'` e `is_projection` em datas futuras;
   parser das notas estruturadas → itens de invoice, usando a gramática do anexo privado) +
   tela Crédito sobre esses itens, incluindo o saldo devedor por contraparte. _(sem o
   parser de notas, a planilha só dá o lump e a tela Crédito nasceria vazia)_
2. Parser OFX/CSV + pipeline normalizar→dedup→classificar→revisão, **incluindo o matcher de
   reembolso real↔previsto (regra 4 + EC15)** (fonte B primeiro: sem dependência externa,
   valida o pipeline inteiro offline).
3. Conector Pluggy/Meu Pluggy (fonte A) reusando o pipeline — **validar aqui os limites de
   conta dev/trial e o fluxo de credenciais no shell Rust** (ver §Rota A).
4. Write-back gated (preview/aprovação/rollback, `sync_batch` + `cell_checksum`) para as
   abas ano.
5. Economia (cálculo + tela + write-back gated, geometria dinâmica ano→bloco→linha).

## Fora de escopo

Iniciação de pagamento; multi-dono de planilha; ML; Mia (slice próprio); conector de
benefício (vale) — saldo manual no bolso restrito (não há conector de operadoras de
benefício no agregador; reavaliar se o segmento aderir ao Open Finance).

## Decisões registradas (validação do dono, 2026-06-12)

- Vale de benefício fica fora da planilha (método), saldo manual no bolso restrito.
- Sync: ao abrir o app (debounce 8 h) + manual sempre disponível; retroatividade = 1º de
  janeiro do ano vigente.
- Pix avulso → Diário, sem limiar — **vigência forward-only** (o histórico em `Saída` não é
  reclassificado; ver EC13).
- Sync nunca escreve na planilha sem confirmação; preview claro; rollback por lote; toggle
  global e por conexão.
- Módulo Crédito dedicado com separação titular/adicional como requisito central.
- **Revisão 2 (review multi-agente, 2026-06-12)**: pré-filtro por tipo de conta nas regras;
  regra 4 (matching de reembolso) + EC15; EC14 (projeções futuras); geometria real da aba
  Economia (blocos-ano lado a lado, farol anual 25 % piso, Economia = aporte deliberado);
  gate do simulador fixado em 12 meses + heurísticas de parcelamento do método; centavos +
  tolerância em toda reconciliação; gramática de notas detectada por ano (sem impor
  cabeçalhos); `sync_log_checksum` ≠ proteção de célula (mecanismo novo `cell_checksum`);
  consentimento Open Finance sem teto fixo de 12 meses (Res. Conjunta BCB/CMN nº 7/2023);
  rota B inclui CSV e mapa de disponibilidade por instituição (anexo privado); tombstone
  diverge intencionalmente do default do Actual Budget.
