# Spec 008 — Import automático (Open Finance + OFX) + módulo Crédito

> Status: **validado com o usuário em 2026-06-12** (instituições, gates, rollback, toggle,
> Crédito dedicado, Economia, Pix→Diário). Detalhes pessoais vivem no anexo privado
> gitignored `.methodology-pack/008-personal-mapping.md`.

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

## Fontes de dados

| Rota                                                                                 | Para quê                                                   | Trade-off                                                                                  |
| ------------------------------------------------------------------------------------ | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| **A. Open Finance via Meu Pluggy** (conta PF gratuita + conta dev linkada via OAuth) | Sync de contas, cartões, transações e faturas              | Melhor cobertura; depende de terceiro; limites de conta dev                                |
| **B. OFX local** (extrato/fatura do internet banking)                                | Fallback 100 % offline-first                               | Esforço manual mensal (~min), parser próprio                                               |
| Benefício (vale)                                                                     | **Sem conector** (benefício fora do Open Finance regulado) | Saldo manual no bolso restrito; checar conector não-regulado do agregador na implementação |

A como caminho feliz, B como fallback, **mesmo pipeline** (a fonte é um adapter).

## Frequência de sync (decisão, baseada no padrão de mercado 2026)

Contexto: o agregador faz auto-sync em janelas de 24/12/8 h e os conectores Open Finance
regulados têm defasagem de **até 24 h**; webhooks (prática recomendada server-side) exigem
endpoint HTTPS — inviável em app local-first sem backend. Portanto:

- **Automático: ao abrir o app**, com debounce (não re-sincroniza se o último sync tem < 8 h);
  re-sync oportunista se o app fica aberto > 8 h.
- **Manual: botão "Sincronizar agora"** sempre disponível (global e por conexão), ignora o
  debounce.
- **Retroatividade do primeiro sync: 1º de janeiro do ano vigente** (alinha com a aba-ano da
  planilha).

**Controle do usuário:** o auto-import tem **toggle global e por conexão** (pausar/desativar/
remover conexão + revogar consentimento). Desativado = o app segue 100 % funcional com
import manual/planilha.

## Pipeline

```
[Pluggy API | arquivo OFX]
  → normalizar (RawBankTxn: provider_id, conta, data, valor, descrição, tipo)
  → dedup (provider_txn_id + checksum; re-sync idempotente)
  → classificar (motor de regras determinístico)
  → persistir (transaction/split/invoice no SQLite)   ← até aqui SEM tocar a planilha
  → recalcular forecast
  → propor write-back → preview + aprovação → Sheets  ← único ponto que escreve fora
```

## Classificação (regras em ordem; primeira que casa vence; auditável)

1. Crédito em conta corrente → `Entrada` (salário, rendimentos, reembolsos). Rendimentos
   diários da mesma origem **agregados por dia**.
2. Débito que casa com despesa fixa conhecida → `Saída` (bloco CONTAS).
3. Débito de pagamento de fatura (casa com fatura fechada de cartão conectado) → `Saída`
   (bloco CARTÕES) e **reconcilia a `invoice`** (status paid).
4. **Demais débitos e Pix avulsos → `Diário`** (decisão do dono: sem limiar de revisão).
5. Transação de cartão de crédito → item da `invoice` aberta (NUNCA Diário); itens do
   **cartão adicional** → `split.owner_person_id` do titular → **Entrada de reembolso
   prevista** no vencimento (net-zero).
6. Correções do usuário viram `classification_rule` (matcher instituição+descrição→destino),
   aplicadas antes das genéricas — categorização que "aprende" sem ML.

## Módulo Crédito (visão dedicada — requisito central)

A planilha só registra o lump; o Neko mostra o que a nota da célula não consegue:

- **Faturas**: todas as faturas (abertas/fechadas/pagas) por cartão, ciclo
  (fechamento/vencimento), total acumulando dia a dia ("velocímetro de crédito").
- **Separação por titular (o mais importante)**: cada item da fatura atribuído ao dono do
  cartão (titular vs adicional); visão "minha parte / parte do(a) parceiro(a)" por fatura,
  com o reembolso previsto linkado.
- **Parcelados**: agenda completa (`n/total`, valor, cartão, término), total comprometido por
  mês futuro — o "salário futuro sequestrado" visível.
- **Limites**: limite, usado e disponível por cartão (dados do agregador).
- **Simulador de compra**: à vista vs parcelado em N× → impacto no forecast encadeado
  (regra do método: parcela só se o fluxo futuro não fica negativo; só bem durável).
  Simulação é cenário, não escreve em lugar nenhum.

## Economia (aba `Economia` — ativar o pilar não usado)

A aba do método é `mês | Entradas | Economia | %` (meta **20–30 %** de poupança).
O Neko passa a calcular automaticamente:

- `Entradas` do mês (já temos), **Economia sugerida** = sobra real do fluxo (ou aporte
  registrado em conta `reserve`), `%` = Economia/Entradas com farol 20–30 %.
- Tela Totais/Reserva mostra a série mensal e a régua; uso de reserva segue a regra
  **usar↔repor** do método (uso = Entrada; repor = Saída futura obrigatória).
- Write-back para a aba `Economia` passa pelo mesmo gate de aprovação.

## Write-back: preview, confirmação e rollback (invariantes)

- **Jamais** escreve na planilha sem confirmação explícita. Não existe modo "auto-aprovar".
- **Preview total** (ApprovalDiffCard): célula a célula — valor antigo → novo, nota antiga →
  nova, com diff textual da nota; agrupado por dia/coluna; aprovar tudo ou por item.
- **Rollback claro**: cada lote aprovado vira um `sync_batch` com snapshot before/after de
  todas as células tocadas; a UI lista os lotes aplicados com **"Desfazer"** (restaura valores
  e notas anteriores, também via gate de confirmação). `sync_log_checksum` detecta edição
  manual concorrente e bloqueia overwrite silencioso.

### Formato escrito (gramática validada contra as notas reais — anexo privado)

- `Saída` = lump; nota com blocos `CONTAS` (`R$ X - Descrição`), `CARTÕES`/`Faturas:`
  (`R$ X - Instituição` ou `Instituição (dd/mm)`), `Investimento:` quando houver.
- `Entrada` = lump; nota `Entradas:` com contrapartes e datas; rendimentos como
  `Rendimentos <Instituição>`.
- `Diário` = soma do dia; a nota de orçamento mensal do dono não é tocada.
- Parcelados mantêm sufixo `n/total` na descrição.

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
3. **Soma bancária == lump da planilha** → itens bancários são gravados como o detalhamento
   e o lump vira `reconciled_by` deles (forecast conta UMA vez; o lump deixa de pontuar).
4. **Somas divergem** → fila de revisão com diff (itens bancários vs lump), o dono decide:
   aceitar detalhamento (write-back propõe corrigir a célula), manter lump (itens ficam
   `shadowed`), ou casar parcialmente.
5. Lançamento manual unitário (não-lump) casa por **valor exato + janela de ±5 dias +
   descrição normalizada**; match → merge mantendo o id do provedor.

Invariante: **uma realidade econômica = um efeito no forecast**, independentemente de
quantas fontes a reportaram.

### EC2 — Pendente → efetivada

Transação de cartão/Pix pode mudar de valor/data ao liquidar (e o provedor pode trocar o
id). Guardar `status` (pending/posted); na efetivação, casar por valor+janela e **atualizar
in-place**, nunca inserir segunda linha.

### EC3 — Ids do provedor instáveis

Reconexão/recriação do item no agregador pode renumerar `provider_txn_id`. Dedup tem
fallback de **fingerprint** (conta + data + valor + descrição normalizada + multiplicidade
no dia) — duas compras idênticas legítimas no mesmo dia não são sobre-deduplicadas porque a
multiplicidade conta.

### EC4 — Transferência entre contas próprias

Banco principal→conta remunerada etc. NÃO é Entrada nem Saída (inflaria os dois lados). Detectar
par (mesmo valor, datas próximas, contas internas) → `transfer` interno, fora do
Entrada/Saída do método.

### EC5 — Estorno, reembolso e compra internacional

Estorno no cartão = item negativo na fatura do ciclo em que cai (reduz o lump futuro).
Compra internacional: IOF/ajuste cambial chegam como itens separados — ficam na fatura como
itens próprios (sem tentar fundir).

### EC6 — Pagamento parcial da fatura / rotativo

Pagamento ≠ total da fatura fechada → invoice fica `partially_paid` com saldo residual
(+ encargos no ciclo seguinte como itens). O match pagamento↔invoice usa tolerância e cai em
revisão se ambíguo.

### EC7 — Parcelados

Parcelas mensais chegam com o mesmo descritor (`n/total` quando o emissor informa). Ligar à
`installment_plan` existente em vez de criar plano novo; antecipação de parcelas remove as
futuras correspondentes (com revisão).

### EC8 — Datas divergentes

Data da compra vs data de lançamento no extrato (D+1, fim de semana): a janela de match
absorve; a data canônica é a do extrato (consistência com o saldo bancário — mesma escolha
do Actual Budget).

### EC9 — Consentimento expirado / conexão quebrada

Consentimento Open Finance expira (12 meses) ou item quebra → estado visível na tela
Conexões ("desatualizado desde X") + aviso no Dashboard de que o forecast pode estar
defasado. Nunca falhar em silêncio.

### EC10 — Transação deletada pelo usuário reaparecendo no re-sync

Exclusões locais guardam o fingerprint num "túmulo" (`tombstone`): o re-sync não ressuscita
o que o dono apagou (configurável, default ligado — espelho da opção do Actual Budget).

### EC11 — Célula editada durante a aprovação do write-back

Já coberto pelo `sync_log_checksum`: o lote só aplica se a célula ainda tem o checksum do
preview; senão, re-gera o diff.

### Testes exigidos (além dos do §TDD)

Golden tests para CADA EC acima; EC1 com os três desfechos (igual, divergente, parcial);
EC3 com multiplicidade 2 no mesmo dia; EC4 com datas D+1; propriedade global: re-rodar o
sync N vezes é **idempotente** (estado final idêntico).

## Modelo de dados (estende, não recria)

- `connection` (instituição, rota A/B, status enabled/paused/revoked, last_sync_at) —
  token/credencial no keyring do SO, NUNCA no banco nem no repo.
- `invoice` (account_id, ciclo, closing/due date, status open/closed/paid, total) +
  `transaction.invoice_id`.
- `installment_plan` (compra parcelada: total, n_parcelas, cartão) + parcelas como
  transações futuras linkadas.
- `transaction.provider_txn_id` (dedup) e `transaction.source` (sheet/openfinance/ofx/manual).
- `classification_rule` (matcher, destino, origem builtin/user-correction).
- `sync_batch` (snapshot before/after para rollback).
- `transaction.status` (pending/posted), `.reconciled_by` (lump↔detalhamento, EC1),
  `transfer` interno (EC4), `tombstone` de exclusões (EC10), fingerprint de dedup (EC3).
- `account.credit_limit` (já existe) populado pelo sync.

## UI

1. **Ajustes → Conexões**: conectar instituição (widget Pluggy) ou importar OFX; status,
   último sync, **toggle por conexão + toggle global**, revogar.
2. **Caixa de revisão**: transações novas com classificação proposta; corrigir ensina o
   motor. Meta < 1 min/dia.
3. **Tela Crédito**: faturas, split por titular, parcelados, limites, simulador.
4. **ApprovalDiffCard + histórico de lotes com Desfazer**.

## TDD obrigatório

Normalização OFX/Pluggy; dedup idempotente; cada regra de classificação; agregação de
rendimentos; ciclo de fatura (closing/due, virada de mês/ano); reconciliação
pagamento↔invoice; reembolso do adicional; agenda de parcelados; simulador (puro);
Economia % e farol; render da nota (golden tests contra a gramática real); snapshot/rollback
de `sync_batch`; conflito por checksum.

## Sequência de implementação (vertical slices)

1. `invoice` + `installment_plan` + tela Crédito sobre dados já importados da planilha.
2. Parser OFX + pipeline normalizar→dedup→classificar→revisão (fonte B primeiro: sem
   dependência externa, valida o pipeline inteiro offline).
3. Conector Pluggy/Meu Pluggy (fonte A) reusando o pipeline.
4. Write-back gated (preview/aprovação/rollback) para as abas ano.
5. Economia (cálculo + tela + write-back gated).

## Fora de escopo

Iniciação de pagamento; multi-dono de planilha; ML; Mia (slice próprio); conector de
benefício (vale) — saldo manual no bolso restrito até existir rota viável.

## Decisões registradas (validação do dono, 2026-06-12)

- Vale de benefício fica fora da planilha (método), saldo manual no bolso restrito.
- Sync: ao abrir o app (debounce 8 h) + manual sempre disponível; retroatividade = 1º de
  janeiro do ano vigente.
- Pix avulso → Diário, sem limiar.
- Sync nunca escreve na planilha sem confirmação; preview claro; rollback por lote; toggle
  global e por conexão.
- Módulo Crédito dedicado com separação titular/adicional como requisito central.
