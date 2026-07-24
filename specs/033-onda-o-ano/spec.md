# Spec 033 — Onda O ano: o tribunal do método

## Contexto

A tela O ano apresentava a visão anual como quatro KPIs (Entradas no ano, Custo de
vida, Performance acumulada, Economizado), um gráfico de barras de performance por mês
e uma tabela de sete colunas (Mês · Entradas · Custo de vida · Diário · Economia · % ·
Resultado · Saldo fim), com uma aba "Comparar anos". A direção da identidade redefine a
leitura: **O ano é a única tela onde o Economizado% pode julgar** — a régua do método é
de _20% a 30% no ano, na média_; no mês ela é só amostra. Por isso a tela abre pela
**faixa**, não por uma tabela, e narra **pela prova**: onde o ano está na faixa, onde
dezembro termina, os doze meses que produziram isso e, por último, o ano em números.

Não há matemática nova de motor: todos os números vêm dos DTOs existentes
(`AnnualMetrics.months`, `Forecast` — `annual_savings`, `month_end`, `coverage`,
`baseline_outflow_cents`, `today`). A lógica de composição — o teste de lastro, os dois
cenários de dezembro, a seleção do estado do veredito, a renda por ano — vive num
**view-model puro** (`src/screens/anoView.ts`) com TDD, no padrão das telas irmãs
(`hojeView`, `tagsView`, `calendarioView`, `lancamentosView`). **Zero mudanças de
backend.**

## Estrutura da tela (ordem do DOM = ordem de leitura)

1. **Cabeçalho de ano** — o título "O ano" vive no shell; a tela abre com o navegador
   de ano (◄ 2026 ►) **no conteúdo**, não no chrome (nasce nos dois viewports de uma
   vez e o veredito tem dono explícito). O crumb da appbar mostra "Onde 2026 está na
   faixa" via store de crumbs do shell.
2. **Veredito** — manchete na voz da marca com o número real como sujeito, sabendo
   dizer má notícia. Seis estados epistêmicos (ver abaixo).
3. **A faixa do método** (card) — o instrumento e assinatura proprietária da tela: a
   régua de escala fixa 0→40% com a zona 20–30 marcada e o ano pousado nela.
4. **Onde dezembro termina** (card) — o "tamanho do buraco" em dois cenários (o
   lançado × o gasto típico), nunca um número solitário.
5. **Os doze meses** (card) — uma linha por mês: trilho = o que entrou, preenchimento
   = o que virou economia, tique = referência dos 20%. Substitui o gráfico de barras.
6. **O ano em números** (disclosure dentro do card dos doze meses) — cada mês é uma
   **linha expansível**, nunca uma tabela. Densidade disponível, nunca imposta.
7. **Sua renda ao longo dos anos** (card) — a comparação que o método manda fazer:
   renda média por mês com registro, ano a ano.
8. **A linha da Mia** (card) — a conversa que o ano puxa, com a ação de abrir a Mia
   sobre o ano.

No desktop, veredito e régua ocupam **largura cheia** (a régua é o instrumento herói e a
leitura pede a largura) e os cards de apoio descem para um **bento de duas colunas
INDEPENDENTES** — massas díspares nunca se alinham por linha (`start` abre buracos,
`stretch` estica card curto sobre tint vazio). No mobile as colunas se dissolvem
(`display: contents`) e os cards fluem na ordem do DOM; o reflow dos doze meses (texto
inteiro numa linha, barra em largura cheia embaixo) usa `order` **apenas na barra
decorativa `aria-hidden`**, para não tocar a sequência de leitura do texto.

## O veredito (seis estados epistêmicos)

O `anoView` seleciona um de seis estados a partir dos dados e do teste de lastro. A cor
de status do método **nunca** aparece aqui — o veredito é texto; o único lugar da tela
que carrega `--warn` é o marcador da régua.

1. **Veredito · fora da faixa, com sobra** (estado real de 2026) — "Você não guardou
   nada em 2026." + "Sobraram R$ {sobra} nos {n} meses que você viveu — e nada virou
   economia. O método pede de 20% a 30% das entradas no ano."
2. **Veredito · dentro da faixa** — "Você guardou {pct}% do que ganhou." + "São R$
   {economia} de R$ {entradas} que entraram. Dentro da faixa do método — dá para seguir
   a vida."
3. **Zero por escolha** (zero-diagnóstico, não falha) — economia zero **com reserva
   íntegra** (≥ 6 meses): "Você zerou a economia para não tocar na reserva." + "Na ordem
   do método, é a troca certa." Sem reserva sadia, o mesmo zero é "não guardou nada" — a
   troca só é certa quando a reserva está de fato sendo protegida.
4. **Estimativa fraca** (o futuro ainda está vazio) — quando há meses suspeitos: "Ainda
   não dá para julgar 2026." + "{k} dos {f} meses à frente preveem sair menos do que
   você costuma gastar, então a projeção do ano não se sustenta. Até confirmar, vale o
   que já foi vivido: {pct}% em {n} meses." A `vlabel` carrega o selo `EstimateMark`.
5. **Sem registro** (o ano nunca foi preenchido) — "{ano} não tem registro." + "Nenhum
   lançamento chegou da planilha para este ano. O ano mais antigo com dados é {y}."
6. **Carregando** — `EmptyState` skeleton (`role="status"`), nunca spinner por cima.

O veredito é **gated pelo lastro**: enquanto houver mês suspeito, o número recua para o
**realizado** (os meses vividos) com o recorte declarado; sem suspeitos, a projeção
anual sustenta o veredito.

## A faixa do método (instrumento)

- **`RangeRuler`** do DS (a onda Este mês selou; O ano reutiliza): escala fixa
  **`max={40}`**, **`zone={from:20,to:30}`**, marcas em 20/30/40%. Escala fixa (não
  relativa ao valor) porque a posição precisa ser comparável de um ano para o outro;
  escala relativa colocaria 0% e 35% no mesmo lugar. O pino satura na borda acima de
  40% e o texto diz o valor real (a barra satura, o número nunca mente).
- **Cor do pino**: única exceção da tela — o marcador assume cor de **status do método**
  (`--warning-400` fora da faixa; `--success-400`/`--primary` dentro), porque é o único
  lugar onde o método julga. O recorte do pino: `label` do `RangeRuler` diz a situação
  ("abaixo/dentro/acima da faixa do método").
- **O recorte declarado**: a régua imprime "nos {n} de 12 meses já vividos" — sem isso
  ela afirmaria o ano medindo sete meses. `RangeRuler` recebe esse recorte no `label`
  do `aria`; a nota abaixo (`--faint`) repete em texto.
- **Nota da régua**: "A régua é **anual**: mês fraco é normal — o que precisa fechar
  entre 20% e 30% é a média do ano. Nos {n} meses vividos, R$ {faltaR} precisariam ter
  saído para a reserva; para {ano} inteiro fechar em 20%, são R$ {faltaA}{— ou R$
  {porMes} por mês de {faixaFuturos}}." As duas grandezas de falta derivam dos mesmos
  ENT/ECO autoritativos (`ENT_R*0.2 − ECO_R` e `ENT_A*0.2 − ECO_A`). Fecho didático
  atrás de "Como funciona?" (`InfoPopover` do termo `economizado`).
- **`pct` de exibição trunca**; o julgamento de estado usa os bps brutos (o número nunca
  contradiz o veredito na fronteira).

## Onde dezembro termina (dois cenários)

- **Cenário lançado** (`DEZ`): o saldo projetado no fim de dezembro (do
  `month_end` do motor). Cor de **dinheiro** (`--money-neg`/`--money-pos`), nunca de
  status — a régua é quem reprova, não este número.
- **Cenário do gasto típico** (`DEZ_TIPICO`): se cada mês suspeito custasse o gasto
  típico em vez do que está lançado. É o contrapeso que impede o número grande de se
  passar por promessa. Só aparece quando há meses suspeitos.
  - `DEZ_TIPICO = DEZ − Σ_suspeitos (baseline − saída_lançada)`; o termo `(baseline −
saída)` é exatamente o `estimated_missing_cents` que o motor já computa por mês
    futuro em `MonthCoverage`.
- **Nota**: a diferença entre os dois é o que ainda não foi lançado; nomeia os meses
  suspeitos, a faixa de saída prevista deles e o gasto típico. **Sem lastro, o bloco
  não porta cor de status do método** — projeção sem lastro não recebe selo de
  aprovação. Rótulo do cenário alternativo: "Se os meses a conferir custarem o de
  sempre" (nunca "Se você gastar o de sempre" — excluiria silenciosamente os meses que
  já têm lastro).
- **Robustez** (achados da revisão externa do desenho): quando o horizonte não alcança
  dezembro, o bloco degrada com honestidade (usa o último mês projetado, nomeando-o) em
  vez de fabricar; sem meses suspeitos a nota afirma que a projeção se sustenta; nenhum
  `R$ ∞` (mínimo de lista vazia ramifica) nem divisão por zero (ano fechado sem futuro).

## Os doze meses

- Uma **linha por mês** via **`Meter`** do DS (ou markup equivalente com a mesma
  política): o trilho é a renda do mês (proporcional ao maior mês), o preenchimento é o
  que virou economia, e o tique marca onde ficam os 20% daquele mês. Doze trilhos com o
  alvo marcado dizem em um segundo o que a tabela leva doze linhas para dizer.
- **Mês vivido** mostra a medição (`{pct}%`); **mês à frente** mostra **"—"**, nunca
  "0%" (zero medido ≠ zero por ainda não ter acontecido — a taxonomia de dado ausente
  do DS). Mês futuro com barra tracejada.
- **Cor de status fica FORA do mês**: quem julga é o ano, então o zero mensal é fato em
  tom neutro, nunca reprovação — **nenhuma linha de mês carrega `--warn`**.
- Mês suspeito leva o selo "Conferir" (pílula tracejada neutra) — pergunta, não acusa.
- Legenda: O que entrou · O que você guardou · Referência de 20% · Ainda não aconteceu.
- **Mês corrente** destacado (nome em `--ink`, peso 600).

## O ano em números (lista, não tabela)

- **Não é tabela.** Sete colunas de dinheiro numa coluna de leitura quebram em qualquer
  viewport e não são o vocabulário da direção — as telas irmãs contam dinheiro em
  **linha**, com o detalhe atrás de um toque. O corte na borda da rolagem de uma tabela
  produzia um caractere fantasma (o "R$" da coluna seguinte lido como glifo solto).
- Cada mês é um **`<details>`**: na superfície o nome e o **resultado do mês** (a única
  grandeza que responde "este mês somou ou subtraiu") com sinal em **cor de dinheiro**;
  abrindo embaixo, **Entrou · Saiu · Economia · Guardado · Saldo no fim do mês**, no
  mesmo trilho de recuo que Tags usa nas exceções.
- Mês suspeito leva "Conferir" na superfície. A fronteira realizado × previsto é dita
  **uma vez** ("Daqui para frente é previsão") antes do primeiro mês futuro, nunca
  repetida em cinco selos.
- Total **Vivido**: {n} meses · entrou R$ {entR} · saiu R$ {saiR} + o resultado somado
  em cor de dinheiro. Nota de rodapé: Entrou e Saiu são tudo que passou pela conta,
  inclusive dinheiro de terceiros; o custo de vida limpo mora em **Tags**.
- `aria-expanded` espelhado nos disclosures (leitores que perdem o marker nativo).

## Sua renda ao longo dos anos

- Uma linha por ano com registro: nome do ano · barra proporcional à média · **renda
  média por mês** · "{pct}% guardado". A média usa só os **meses com registro** —
  dividir por 12 inventaria uma queda que não existiu (um ano preenchido a partir de um
  mês tardio). Para o **ano corrente**, "meses com registro" = meses vividos; para anos
  passados, meses com qualquer dado.
- `sobra`/`renda` são **fluxo** (entrou − saiu / entrou no período), nunca o saldo de
  fechamento — grandezas diferentes, léxico separado (estoque × fluxo).
- Rodapé: o delta de renda entre o penúltimo e o último ano com dados + a leitura de que
  ganhar mais não vira economia sozinho quando o guardado seguiu em zero.

## Teste de lastro (regra do método desta tela)

O autor julga o ano **projetado**, mas projeção só vale com lastro. Um mês à frente
sustenta o veredito quando a saída lançada ≥ **60% do gasto típico** (mediana das saídas
totais dos meses vividos). Abaixo disso o mês entra como **suspeito**, não como falso:
tem lançamento, só tem pouco — pode ser mês legitimamente barato ou pode faltar lançar.
A tela pergunta (`Conferir`), não acusa. Enquanto houver mês suspeito, o veredito recua
para o realizado.

- **Reconciliação com o motor** (regra 6 do `ui-standards`: a copy fala o que o motor
  calcula): a saída total do mês é `sai = income − performance`; o gasto típico é
  `mediana(sai dos meses vividos)`, idêntico por definição ao `baseline_outflow_cents`
  do motor (mediana da saída total realizada); o piso de 60% é o limiar de confiança
  desta tela, no view-model e testado. Um mês futuro é suspeito quando `sai < 0.6 ×
gasto_típico`.

## Regras que descem para o DS / ui-standards

- **Cor de status do método é privilégio do agregado que o método julga.** O zero
  mensal é fato em tom neutro; nenhuma linha de mês carrega `--warn`.
- **Todo display declara o próprio recorte** (a régua imprime "nos {n} de 12 meses").
- **Projeção sem lastro não recebe cor de aprovação** — ponto de status verde exige
  lastro; sem ele, selo de estimativa e cenário alternativo ao lado.
- **Léxico separado para estoque e fluxo** ("sobrou/entrou" = fluxo; "saldo" = estoque).
- **Contorno sólido em faixa de status** (a faixa diluída em transparência falhava o
  contraste sobre o trilho claro).
- **Cor de dinheiro (`--money-pos`/`--money-neg`) é o terceiro eixo**, separado de marca
  e de status; sinal aritmético nunca usa `--warn`.
- **Nenhum rótulo de interface começa em minúscula** (regra 5, ampliada).
- **Tabela não é o vocabulário da direção**; a forma canônica é linha com disclosure.

## Reuso e o que muda no código

- **`AnnualScreen.tsx`** reescrita na anatomia acima; **`ano.css`** reescrita namespaced
  (`.ano__*`). **`anoView.ts`** novo (view-model puro) + `anoView.test.ts`.
- Reuso do DS: `RangeRuler` (a régua), `Meter` (linhas de mês / barras de renda),
  `EmptyState` (carregando/erro), `HealthBadge` (opcional no veredito), `InfoPopover`
  (didática), `Money`/`SignedMoney`, `EstimateMark`, `NoRecordDash`, `setCrumb`,
  `useCommand`, `SR_ONLY`. Limiar `SAVINGS_MIN_BPS` de `totaisStatus.ts` como fonte
  única do piso de 20%.

## O que morre

- Os quatro KPIs (Entradas/Custo de vida/Performance acum./Economizado) — os números
  renascem no veredito, na régua e no "ano em números".
- **"Performance acumulada" some do produto**: não existe no método — o que existe é o
  saldo crescendo, e o autor declara que, ao começar a economizar, a performance deixa
  de importar. O eixo do "onde dezembro termina" a substitui como leitura do ano.
- O gráfico de barras de performance (sucedido pelos doze meses).
- A tabela de sete colunas (sucedida pela lista de disclosure).
- A aba "Comparar anos" e o `SegmentedControl` — a comparação vira **renda ao longo dos
  anos** (o que o método manda comparar), sempre visível, sem aba.

## Motion

Coreografia de entrada única por montagem (fade+translate curto com stagger, CSS puro,
`prefers-reduced-motion` respeitado, teto de 480ms do DS). Superfícies transitam;
**dinheiro e percentuais nunca animam** (sem count-up; o pino da régua não desliza ao
montar).

## Acessibilidade

- Hierarquia: título no shell; h1 do veredito (`data-large-title`); h3 por card. Cards
  como `section` com `aria-labelledby`.
- Régua e barras dos meses com texto equivalente completo (`role="img"`); as doze barras
  não duplicam em `aria-label` o texto vizinho (ficam `aria-hidden`).
- Texto e `aria` dizem o valor verdadeiro (truncamento e saturação só de exibição).
- Navegação de ano com alvos ≥ 44px (área por pseudo-elemento, sem inchar a silhueta);
  `aria-expanded` espelhado; `role="status"` no esqueleto.
- Contraste AA nos 2 temas × 6 acentos; ordem do DOM = ordem de leitura.

## Fora de escopo

- Mudanças de backend (todos os dados já existem nos DTOs).
- As demais ondas (Teto do diário #206, Horizonte #207) — reutilizam a régua e a grade
  de mês desta e das irmãs.
- Override manual de modo de gasto (decisão do #178 mantida).

## Fidelidade ao método (traço às fontes)

Cada regra abaixo foi verificada contra as fontes primárias do método antes de virar código;
as fontes são locais e não versionadas, então aqui fica só a regra e o que ela decide.

- **A régua é anual, 20–30% na média.** O agregado do ano é `% do ano = Σeconomia ÷
Σentradas` — nunca a média das taxas mensais.
- **"Performance acumulada" não existe no método** — não há soma anual de performance; a
  hierarquia é economia acima de performance a partir do momento em que se começa a guardar.
- **Zerar a economia para proteger a reserva é a troca certa** — o zero vira diagnóstico, não
  falha, quando a reserva permanece íntegra.
- **A comparação entre anos é de RENDA**, não de performance: média de entradas por mês com
  registro, ano a ano.
- Os números de referência da verificação (gasto típico, piso de lastro e quais meses
  reprovam) saem da planilha do próprio usuário em tempo de execução; o view-model é testado
  contra uma fixture equivalente em `anoView.test.ts`.

## Aceitação

1. A tela abre por **veredito + régua da faixa** (0→40, zona 20–30, pino com recorte
   declarado e cor de status só no marcador), não por KPIs nem tabela.
2. **Onde dezembro termina** em dois cenários quando há meses suspeitos, com cor de
   dinheiro (nunca status), robusto a horizonte curto / ano fechado / sem suspeitos.
3. **Os doze meses** como linhas com trilho+economia+alvo 20%; mês futuro "—" e barra
   tracejada; nenhuma linha carrega cor de status.
4. **O ano em números** como lista de disclosure (não tabela), com resultado do mês na
   superfície e o detalhe atrás de um toque; fronteira previsto dita uma vez.
5. **Sua renda ao longo dos anos** com média por meses com registro (corrente = vividos)
   e "{pct}% guardado".
6. Os **seis estados epistêmicos** do veredito cobertos; veredito gated pelo lastro.
7. `anoView.ts` puro e testado (fixture da planilha real reproduz os números aprovados);
   nenhuma mudança de backend.
8. Gates: `npm run check` verde; react-doctor sem achados novos; e2e visual com
   baselines regenerados do zero (2×), consumidores irmãos inspecionados; impeccable
   audit + critique; copy sentence-case; dinheiro tabular e estático.
