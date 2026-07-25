# Spec 037 — Onda Mia: a conversa com recibo auditável

## Contexto

A direção do refresh chama-se "Conversa com a Mia" e esta é a tela que dá
nome a ela — a última onda por tela antes da firma. A `CopilotScreen` atual
é pré-onda: semeia uma pergunta que a pessoa nunca fez, imprime o cálculo
como bloco monoespaçado, não tem marco de dia/hora/autoria, deixa 400px de
vazio entre a conversa e o composer, e oferece duas sugestões que o produto
não sustenta — uma pede análise por categoria (que o método rejeita) e outra
promete pré-lançamento em lote (fora do escopo v1 do copiloto).

Onda **frontend-only**: nenhum runtime de LLM entra aqui — ele tem trilho
próprio. O que esta onda entrega é a **forma** da conversa e o repertório
determinístico que já cabe nos DTOs existentes (`get_dashboard_summary`,
`get_forecast`): a tela responde do próprio motor, com a conta à mostra, e
recusa honestamente tudo o que não alcança. Quando o runtime chegar, ele
publica nesta mesma forma — o recibo é o contrato visual da resposta.

Toda derivação (intenção, resposta, recibo, recusa) vive num view-model puro
novo (`miaView.ts`, TDD). A tela é superfície e wiring.

## A assinatura: o recibo auditável

Toda resposta com número traz um **recibo** — não um bloco de log, mas a
conta impressa:

- Linhas de **operando**: rótulo à esquerda, valor tabular à direita.
- Um **operador** na margem (`÷`, `−`, `mín`) entre os operandos, em mono
  apagado — a operação fica visível, não subentendida.
- Uma linha de **resultado**, com tinta forte, o valor e o estado do método
  quando existe ("— em paz", "— fora da faixa").
- **Proveniência** no pé, na voz da direção: "Cálculo determinístico · Lê
  sua planilha · Responde local". Resposta didática troca a linha por
  "Explicação do método" — nunca alega cálculo onde não houve.

O recibo é o que torna a regra 6 do `ui-standards` literal: a frase não
descreve a fórmula, ela **imprime** a fórmula que o motor computou. Nenhum
número do recibo nasce aqui: todos vêm dos DTOs.

## Estrutura da tela (ordem do DOM = ordem de leitura)

1. **Thread** — a conversa (`role="log"`, `aria-live="polite"`), com marco de
   dia, autoria e hora.
2. **Painel "Os números por trás"** — os fatos que a Mia usa, sempre à vista.
3. **Dock da tela** — sugestões, composer e a linha de honestidade.

Desktop: duas colunas — conversa com teto de leitura (`720px`, centrada) na
coluna 1, painel `312px` sticky na coluna 2; o dock ocupa a linha 2 da coluna
1, **sticky na base do scroller**, com véu de fundo para a thread não vazar
por baixo. Mobile: coluna única; o dock segue sticky na base, e o espaço do
dock flutuante do app é o próprio respiro do scroller. O painel sai (abaixo).
A sequência do DOM não muda por viewport, só a visibilidade e a coluna
(regra 10).

A rolagem automática vai até o fim do **scroller**, não até o fim da tela:
um composer ancorado flutua enquanto sobra rolagem, e parar no fim de si
mesma deixaria a resposta nova nascer por baixo dele.

Moldura é de quem age: bolha da pessoa, resposta da Mia, painel e composer
têm superfície; a thread não tem caixa em volta (regra 22).

## O painel "Os números por trás" — desktop

O painel é o **índice do repertório com os valores vivos**: pode gastar hoje ·
fim do mês previsto · Economizado no ano · reserva · ponto mais baixo da
estrada · faturas em aberto (as duas últimas só quando existem). Cada linha é
**um botão que faz a pergunta correspondente** — o painel é atalho de conversa,
não um segundo lugar onde o número mora. É por isso que ele pode ser
desktop-only sem violar a regra 8: nenhum fato dele é exclusivo do desktop —
cada um é alcançável por uma pergunta do repertório (em qualquer viewport) e
por uma tela. Desvio consciente registrado; a razão é ergonômica (regra 20:
densidade de mouse).

Linha em estado fora da régua usa warning (âmbar), nunca vermelho
moralizante; a nota do pé — "Cálculos determinísticos, nunca inventados." —
fica.

## Repertório determinístico (6 cálculos + didática)

Nenhuma conta nova de método nasce nesta onda: cada resposta lê os DTOs e
imprime a operação que o motor já fez.

| Pergunta                       | Fatos                                                                                                         | Recibo                                                                  |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| Quanto posso gastar hoje?      | `safe_to_spend_today_cents`, `binding_guardrail`, `savings_headroom_cents`, teto + procedência, gasto de hoje | limite que morde `mín` teto do diário · já gasto hoje · livre pelo teto |
| Como o mês está indo?          | `MonthMetric` do mês corrente, `balance`                                                                      | entradas `−` saídas e economia `=` performance · custo de vida na nota  |
| Como está a economia do ano?   | `annual_savings` (régua, estado, alvo)                                                                        | economia da régua `÷` renda realizada `=` Economizado%                  |
| Como está a reserva?           | `reserve_months`, `reserve_state`, `reserve_basis_months`                                                     | meses cobertos · meta de 6 meses                                        |
| Tem buraco na estrada?         | `deepest_deficit`, `horizon_end`                                                                              | menor saldo do horizonte · o dia · a faixa do termômetro                |
| Quando vence a próxima fatura? | `upcoming_invoices`, `next_fatura_*`                                                                          | uma linha por cartão do vencimento `=` total                            |

A conta da performance imprime o total que a régua desconta, não a lista de
buckets: as máscaras por régua podem separar o que entra em "custo de vida" do
que entra na Performance, e uma soma de parcelas que não fecha é uma fórmula
mentindo em prosa (regra 6). O custo de vida sai na nota, como o fato que é.

A voz é a mesma das telas (regra 4 — uma invitação por estado): o guardrail
que morde é nomeado com a frase da Hoje ("Sem tocar na economia planejada do
ano." × "Sem deixar nenhum dia no vermelho."), o teto é sempre o **segundo**
limite do dia, o termômetro usa as faixas absolutas da planilha, e a régua
do ano é anual por definição — um mês fraco não reprova o ano.

**Didática** (`o que é buraco do futuro / termômetro / diário / performance /
custo de vida / economizado`…): resposta em texto na voz do método, sem
recibo, com proveniência "Explicação do método". Os termos e a redação são os
do glossário que a UI já usa — a Mia não inventa um segundo vocabulário. O
glossário sai do arquivo do `InfoPopover` para um módulo próprio
(`design-system/glossary.ts`) e ganha três verbetes que faltavam: buraco do
futuro (a redação já aprovada no Horizonte), termômetro e Diário.

## Recusa honesta — as quatro portas

Recusar é resposta de primeira classe, e cada motivo tem saída concreta:

1. **Sem dado** — a pergunta é do repertório, o dado não existe (sem reserva
   mapeada, sem fatura registrada, sem teto estipulado). Rende o estado
   epistêmico (`NoRecordDash`/`EstimateMark`, nunca um zero fabricado) e o CTA
   que registra o que falta.
2. **Fora do que a conversa alcança hoje** — texto que não casa com o
   repertório. A tela **não finge**: diz que a conversa aberta ainda não está
   ligada, lista o que ela responde agora e devolve as sugestões.
3. **Capacidade não suportada** — a intenção é reconhecível mas não é da
   tela: registrar por conversa (o registro é o gesto global de Lançar),
   editar/apagar lançamento, pré-lançar em lote. Nomeia o caminho certo.
4. **Ambígua** — o texto casa com duas intenções. Pergunta de esclarecimento
   com as duas opções como pílulas; nunca supõe.

Caso didático dentro da porta 3: pedido de gasto por categoria ("onde gastei
mais?"). A resposta **ensina** em vez de recusar seco — o método não orça por
categoria, a tag é interruptor de régua — e abre a porta de Tags. É a única
resposta da tela que corrige a pergunta, e existe porque a pergunta é
familiar o bastante para reaparecer todo dia.

## O estado inicial é o mais desenhado

Sem conversa, a tela é a saudação do gato: avatar grande, saudação pela hora
do dia (a mesma função da Hoje) e uma frase de identidade que diz o que ela
faz e como responde. Nenhum parágrafo didático permanente (regra 1): a
profundidade vive nas sugestões e nas respostas.

No desktop, saudação, sugestões, composer **e o painel** viajam juntos para o
meio da tela — um composer ancorado com 400px de vão acima lê como sobra de
layout, não como composição (é também o padrão dos assistentes de 2026), e um
painel ancorado no topo viraria uma ilha solta ao lado. No polegar o composer
não sobe (a mão alcança a base), mas a saudação centraliza no espaço que sobra:
o mesmo vão morto no dispositivo primário seria o defeito que esta onda existe
para matar (regra 18). O campo recebe foco de largada só em teclado físico —
no polegar o autofoco abriria o teclado virtual por cima da saudação.

As sugestões são **pílulas roláveis** no mobile (`scroll-snap-type: x
mandatory`, sem barra, alvo ≥ 44px; regra 19). No desktop aparecem as quatro
primeiras — as demais seriam três linhas de pílula empurrando o composer, e
lá o painel já oferece cada pergunta com o valor ao lado. São exemplos do
repertório — não contrato de capacidade.

## Marcos de dia, hora e autoria

- **Marco de dia** (`Hoje`, `Ontem`, data por extenso adiante) entre blocos de
  dias diferentes — derivação pura com `today` injetado.
- **Hora** em cada mensagem (`HH'h'mm`), no pé da bolha da pessoa e na linha de
  proveniência da Mia.
- **Autoria** pelo avatar do gato + nome acessível na linha ("Mia:" para
  leitor de tela); a mensagem da pessoa é anunciada como "Você:".
- A conversa vive na **sessão** (memória do módulo): sobrevive à navegação
  entre telas, morre ao fechar o app. Transcript persistido e apagável é
  contrato do runtime — inventar um store paralelo agora seria dívida com
  regra de privacidade própria. A linha de honestidade do dock declara isso o
  tempo todo ("A conversa fica só nesta sessão"), não só na primeira mensagem.

## Motion

Mensagem nova entra com rise + fade curtos (padrão das ondas); o dinheiro
dentro dela **não anima** (regra 28). A rolagem até a última mensagem leva o
**scroller** ao fim — parar no fim da própria tela deixaria a resposta por baixo
do dock ancorado (a armadilha virou a regra 21 do `ui-standards`, escrita nesta
onda). `prefers-reduced-motion` desliga a entrada e a rolagem suave.

## Divergências entre o desenho e as réguas do repositório

1. Copy minúscula do protótipo capitaliza na fronteira (regra 5): "os números
   por trás" no corpo do painel vira título "Os números por trás".
2. Os botões de **anexo** (imagem, foto do comprovante) do protótipo morrem:
   não existe capacidade de leitura de imagem em lugar nenhum do produto, e
   um botão que não faz nada é promessa falsa.
3. A pílula "Registra R$ 4,50 do café" morre: proposta com aprovação fora do
   loop é contrato do runtime; simular registro pelo chat criaria um segundo
   caminho de escrita sem o ledger de propostas.
4. As sugestões "Onde gastei mais?" e "Pré-lançar o próximo mês" morrem —
   a primeira pede a análise que o método rejeita (regra 2: pergunta que o
   método rejeita não ganha porta), a segunda está fora do escopo v1.
5. O badge "Lê sua planilha · responde local" sai do título e vira a linha de
   proveniência do recibo, onde a afirmação é verificável frase a frase.
6. O painel do protótipo é decorativo (só leitura); aqui cada linha vira
   pergunta — sem isso ele seria conteúdo exclusivo do desktop.

## Fidelidade ao método

Tudo que a tela afirma já é lei do motor: o "pode gastar" é o guardrail que
morde e o teto é o segundo limite do dia; o Economizado% é anual e a régua
inclui previdência só com reserva de 6 meses; custo de vida é Saídas + Diário

- Cartão; o menor ponto do horizonte é a prova do "pode gastar"; o termômetro
  tem faixas absolutas. A Mia **expõe** e ensina — não recalcula, não julga.

## O que morre

- A conversa semeada (pergunta fabricada em nome da pessoa).
- `.mia-calc` como bloco mono de log → recibo.
- As classes globais `.card`, `.card__head`, `.card__title`, `.card__body`
  redefinidas em `mia.css` (já vivem em `redesign.css` — colisão) e o par
  `.xs`/`.xs-title`, exclusivo desta tela.
- O esqueleto artesanal de carregamento → `EmptyState` (regra 16).

## Fora de escopo

- Runtime, provedor, fachada de ferramentas, consentimento, transcript
  persistido, propostas de lançamento (trilho próprio do copiloto).
- Mudanças de shell/dock, backend, forecast, write-back.

## Aceitação

- `npm run check` verde; TDD do `miaView` (roteamento de intenção, cada
  recibo, as quatro recusas, marcos de dia, estados epistêmicos) verde.
- Baselines visuais regenerados do zero (server fresco, duas passadas) e
  inspecionados nos dois temas + mobile; e2e cobre o estado inicial e uma
  resposta com recibo.
- React Doctor sem novos achados; impeccable audit + critique sem P0/P1
  pendentes; copy sentence-case; WCAG AA nos dois temas; dinheiro não anima;
  cor de status nunca segue o acento.
