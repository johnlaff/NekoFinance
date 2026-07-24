# Spec 036 — Onda Cartões: o sub-ledger de faturas

## Contexto

O domínio do cartão (spec 026) aterrissou o contrato inteiro — conta-cartão com
aliases, fatura persistida cartão×ciclo, séries, reembolso vinculado, gate de
2 pernas — e a tela `CartoesScreen` expõe tudo de forma funcional, mas
pré-onda: sem herói, sem assinatura própria, formulário com estilo inline e o
drill escondido atrás de um `Disclosure`. Esta onda traz a tela para a direção
"Conversa com a Mia": o desenho aprovado da direção 7 vira código de produção.

Onda **frontend-only**: zero mudanças de backend. Todo número exibido já
existe nos DTOs (`Card`, `InvoiceSummary`, `InvoiceDetail`, `CardProposal`,
`get_dashboard_summary`); a composição visual e as derivações de exibição
vivem num view-model puro novo (`cartoesView.ts`, TDD).

## Estrutura da tela (ordem do DOM = ordem de leitura)

1. **Banner de proposta** (quando houver) — a Mia farejou um cartão.
2. **Gate do modo cartão** — as 2 pernas com números e o convite didático.
3. **Lista de cartões** — um card-face por titular; adicionais aninhados.
4. **Detalhe da fatura** do cartão selecionado — seletor de ciclo, herói,
   histórico em barras, totais + reconciliação, compras, séries, reembolsos,
   recorte por dono, líquido de conferência.
5. **Formulário** de cadastro/edição (estado sobreposto à lista).

Desktop: bento de 2 colunas independentes (lista `340px` | detalhe `1fr`;
ultrawide ≥ 1700px sobe o teto para `1280px` e a lista para `380px`),
regra 9 do `ui-standards`. Mobile: lista → drill como estado da tela — tocar
num cartão revela o detalhe com "Voltar" no topo; o alvo do scroll carrega
`scroll-margin-top` do tamanho do appbar fixo, senão a única saída do drill
nasce coberta por ele. A sequência do DOM não muda por viewport, só a
visibilidade (regra 10). O shell não muda: Cartões segue no menu "Mais" do
dock — a composição do dock é decisão da fundação.

Moldura é de quem age: a proposta (banner acionável) e o painel do drill têm
borda; o gate rende como linha nua sob o veredito e a meta do cartão como
bloco de fundo sutil — quatro caixas consecutivas leriam como formulário
(regra 21).

## Card-face (assinatura da tela)

Face de acento calma — o único lugar do app onde o acento vira superfície:

- Gradiente do desenho aprovado sobre `--accent`, tinta `--accent-ink`
  (par atômico por paleta); marca NekoMark discreta; chip decorativo.
- Nome do cartão, "Titular · {dono}", e a **próxima fatura**: valor tabular
  (`Money`), "Vence {dd de mmm}", chip de status e "Fecha em N dias" quando
  aberta.
- O titular selecionado rende como face; os demais titulares rendem como
  linha compacta (nome, ciclo, status ou travessão) e são clicáveis para
  trocar a seleção. Adicional aninhado sob o titular com `OwnerChip`
  ("Herda o ciclo do titular · sub-fatura própria").
- **Limite é exibição discreta** na linha de meta ("Fecha dia 20 · vence dia
  10 · limite R$ 12.000") — nunca barra, nunca cor (spec 026 D1).
- Cartão sem fatura: `NoRecordDash` + "Sem fatura registrada ainda."

## O herói da fatura

- Valor grande tabular (o herói tipográfico da coluna), subtítulo **honesto
  com a autoridade** (regra 6): `stated_total` presente → "Total declarado —
  autoridade da planilha"; ausente → "Soma das compras itemizadas".
- Vencimento ("Vence 10 de ago") + estado do ciclo ("Fecha em 5 dias" na
  aberta; "Fechou em {dd de mmm}" na fechada; "Fecha em {dd de mmm}" na
  prevista — a data de abertura do ciclo não é dado do contrato, e inventá-la
  seria mentir; "Paga em {dd de mmm}" na paga) — derivação pura com `today`
  injetado.
- **Chip de status com cor fixa** (nunca segue o acento): `paga` → success,
  `fechada` → warning, `aberta` → tinta forte neutra, `prevista` → neutra
  apagada. O mapeamento atual `aberta → Badge primary` (acento) morre.

## Seletor de ciclo

- Segmentado com ≤ 6 ciclos (velho → novo), cada opção com o mês e o status
  ("Ago · Aberta"); seleção default = fatura aberta, senão a próxima a vencer,
  senão a mais recente. A janela é **ancorada na seleção default**: séries
  longas materializam muitas previstas à frente, e "os últimos 6" deixariam a
  aberta sem rádio nem barra — a âncora senta na penúltima posição (até 4 de
  história + a âncora + 1 prevista).
- Trocar de ciclo recarrega o detalhe; o remanejo de compra ("Mover para o
  ciclo anterior/seguinte") permanece.

## Histórico em barras

- Barras das últimas ≤ 6 faturas (`effective_total_cents`), alturas
  normalizadas pelo máximo da janela; a do ciclo selecionado em acento, as
  demais em tinta neutra fraca. **Dinheiro nunca anima** — barras estáticas.
- Acessível como `role="img"` com equivalente textual completo (mês → valor),
  não `aria-hidden` como no protótipo.
- Legenda: "Faturas dos últimos N ciclos — a de {mês} ainda acumula." (a
  segunda oração só quando a selecionada está aberta).

## Totais + reconciliação

- A linha-cabeça nomeia a mesma autoridade do herói: "Total declarado" quando
  o declarado existe, "Compras itemizadas" quando o efetivo é a soma (e aí a
  segunda linha não repete o mesmo número). Com declarado + compras: "Total
  declarado" / "Compras itemizadas" e, quando houver delta, a linha de
  reconciliação: "Não itemizado — parte da fatura sem linha" (tracejada,
  itálica) — **nunca vira item** (spec 026 D3).
- **Sem nenhuma compra itemizada** — o modo dominante da planilha real, que
  registra a fatura como lump por cartão — a leitura colapsa numa frase:
  "Registrada como valor único — sem compras itemizadas neste ciclo." Nada de
  "Compras R$ 0,00" (zero fabricado) nem reconciliação do valor inteiro
  (ruído sem informação além do próprio total).
- "Ajustar total declarado" mantém o gesto atual (input + confirmar), com o
  rótulo completo, nos dois modos.

## Compras

- Linha: descrição (+ `n/N` mono quando parcela), subtítulo com a data e —
  **só quando o dono diverge do titular** — o `OwnerChip` (pill em toda linha
  não é informação, regra 24); valor tabular à direita.
- Remanejo por ícones ‹ › com `aria-label` nomeando o ciclo de destino e
  alvo de toque de 44px (área crescida por margem negativa, regra 19).
- "Confirmar"/"Salvar" com valor BRL inválido respondem com erro inline
  (`role="alert"`) — nunca falham em silêncio.

## Séries

- **Parcelado**: título + valor da parcela, progresso via `Meter` do DS
  (regra 15) com fração `n/N`, legenda "Parcela n de N · faltam R$ X em K
  faturas" (K = N−n; X = valor×K) — derivação pura testada.
- **Assinatura**: título + valor + "Todo mês, dia D · pré-lança nas faturas
  futuras" (D = dia da ocorrência exibida).
- Gestos mantidos: editar (regenera futuras ocorrências) e cancelar
  assinatura a partir do ciclo.

## Reembolsos vinculados

- Linha com "+R$" em success e chip "Prevista" quando `is_projection`;
  legenda "Entra como Entrada no vencimento — a régua julga o valor cheio".

## Recorte por dono

- Titular + cada sub-fatura (`OwnerChip` + valor) + "Total do emissor"
  (derivado, nunca persistido). Fecho da coluna: "Líquido de reembolsos" com
  chip "Conferência" e o `InfoPopover` atual (a régua julga o bruto).

## Gate do modo cartão

- As 2 pernas com números vivos e o "quanto falta" (comportamento atual), em
  linha de chips calma; a perna abaixo do alvo usa warning (âmbar) — nunca
  vermelho moralizante. A didática segue atrás de "Como esta leitura
  funciona" (`InfoPopover`), com o convite de testar 2–3 meses no débito.

## Banner de proposta

- Farejo da Mia: cabeça "A Mia farejou um cartão na planilha", corpo citando
  a seção CARTÕES do mês de origem e o alias que não casou, ações "Cadastrar
  cartão" / "Dispensar" (aceite explícito, spec 026 D6).

## Formulário

- Sem mudança de campos ou validação; os estilos inline (`FORM_FIELD`,
  `LABEL`) viram classes em `cartoes.css`.

## Motion

- Entrada da tela com rise/fade sutil das superfícies (padrão das ondas),
  `prefers-reduced-motion` respeitado; números nunca animam; a troca
  lista→drill no mobile é corte seco ou fade curto — sem slide de página.

## Divergências entre o desenho e as réguas do repositório

1. Copy minúscula do protótipo capitaliza na fronteira (regra 5): "fecha em
   5 dias" → "Fecha em 5 dias"; os status do seletor de ciclo abrem linha →
   "Paga", "Fechada", "Aberta", "Prevista".
2. A pill "reembolso" numa linha de compra não existe no contrato — o DTO não
   vincula compra↔reembolso; o vínculo é da fatura. Morre; a seção
   "Reembolsos vinculados" carrega a informação.
3. A nota "A Mia detectou esta cobrança recorrente" numa série não tem campo
   de origem no DTO — sem dado, a frase seria fabricada. Morre.
4. O status "Aberta" no protótipo usa a tinta do acento sobre o card-face por
   contraste; fora dele a produção usa tinta neutra forte — cor de status
   nunca segue o acento (regra 27).
5. O dock do protótipo mostra Cartões; a fundação coordena o dock e Cartões
   vive no "Mais" — o shell fica intocado.
6. As barras `aria-hidden` do protótipo ganham equivalente textual
   (`role="img"`) — WCAG.
7. Sidebar e rodapé do protótipo são chrome do shell, fora da onda.

## Fidelidade ao método

Tudo que a tela afirma já é lei do motor (spec 026): fatura é lump por cartão
que vira Saída no vencimento; limite nunca é régua; reembolso volta como
Entrada no vencimento e as réguas julgam o bruto; o gate observa economia
20–30% viva e reserva de 6 meses, guiando sem punir. A onda expõe — não
recalcula nada.

## O que morre

- O `Disclosure` "Faturas" (o drill vira coluna própria / estado mobile).
- `statusTone` com `aberta → primary` (acento em status).
- `FORM_FIELD`/`LABEL` inline.
- A demo pobre do fallback web — enriquecida para exercitar barras (≥ 4
  ciclos), parcela com progresso, assinatura, reembolso previsto e
  sub-fatura, com nomes neutros.

## Fora de escopo

- Registrar compra pela tela (o registro vive no gesto global de Lançar).
- Mudanças de shell/dock, write-back, backend, forecast.

## Aceitação

- `npm run check` verde; TDD do `cartoesView` (barras, ciclo default, herói
  honesto, progresso de parcela, rótulos de estado do ciclo) verde.
- Baselines visuais regenerados do zero (server fresco, 2 passadas) e
  inspecionados nos 2 temas; e2e atualizado para o novo fluxo lista→drill.
- React Doctor sem novos achados; impeccable audit + critique sem P0/P1
  pendentes; copy 100% sentence-case; WCAG AA nos 2 temas × paletas; dinheiro
  não anima; cor de status fixa.
