# Spec 039 — Firma do refresh: assinaturas, contrato de motion e checklist

## Contexto

O refresh de identidade sob a marca Neko sistematizou o design system "Midnight Purr"
(spec 024) e aterrissou onze ondas por tela. Esta spec fecha o aceite transversal: o que
nenhuma onda isolada enxerga — a consistência das assinaturas proprietárias através das
telas, a linguagem de motion sob o contrato do DS, e o somatório do checklist de identidade.

O aceite exige duas coisas: resolver os oito achados do diagnóstico de identidade e definir
de três a cinco assinaturas proprietárias. Assinatura aqui tem definição operante: **um
momento reconhecível** — proprietário, repetido com disciplina, e notável quando aparece.

A fundação do DS não carrega identidade sozinha. Zinc, Geist e geometria de pílula são as
escolhas de maior convergência do mercado; a diferenciação do produto mora inteira nas
assinaturas, não nos tokens. Elas não são acabamento do refresh — são a parte dele que
diferencia.

## Decisões

### D1 — Princípio não é assinatura

Didática atrás de pergunta (`InfoPopover`, presente nas onze telas) e honestidade epistêmica
(`EstimateMark`/`NoRecordDash`/`ModeChip`, nove telas) são **princípios canônicos**, não
assinaturas: onipresentes e invisíveis por desenho, que é o oposto de um momento. Já vivem
como regra dura (`docs/ui-standards.md` 1 e 16) e ficam fora da contagem.

### D2 — Assinatura 1: veredito-primeiro, um contrato em duas escalas

O contrato — o que faz o veredito ser assinatura em qualquer tamanho:

- A palavra vem antes do número.
- O estado é sempre texto; cor nunca é o único sinal.
- O nível vem de um vocabulário fechado, único no app.

Duas escalas implementam o contrato, e são escalas distintas de propósito — um título que
abre a tela e uma pílula dentro de um bloco não são o mesmo componente:

- **`VerdictHero`** (novo no DS) — abre a tela: olho, título grande, corpo. Consumidores:
  O ano, Horizonte, Cartões.
- **`HealthBadge`** (existente) — marca um bloco.

A faixa tintada com ícone dos Cenários é absorvida pelo `HealthBadge`: caixa colorida para
ranquear conteúdo é problema de escala tipográfica, não de cor (`ui-standards` 23), e
pulveriza acento (24). O app fica com duas formas de dar veredito, não três.

### D3 — Assinatura 2: recibo auditável

O recibo imprime a conta: cada operando numa linha, o sinal da operação na margem, o
resultado destacado. É a resposta do app para "de onde veio esse número". Extraído da tela
da conversa para o DS, sob três travas que o impedem de virar sobrecarga:

- **Substitui, não soma.** Onde entra, o recibo toma o lugar da prosa que descreve a
  fórmula. A frase deixa de poder divergir do motor porque deixa de existir — a regra 6 do
  `ui-standards` para de depender de vigilância.
- **Fronteira declarada entre as portas.** "Como funciona?" (`InfoPopover`) responde o que o
  conceito significa no método — invariável. "Ver a conta" (recibo) responde de onde este
  número saiu hoje — aritmético. Conteúdo que não cai claramente numa das duas não vira
  nenhuma.
- **Um por tela**, no número herói. Número derivado ganha recibo; número que o usuário
  digitou não tem conta a mostrar.

A preferência "Conta sempre à mostra" deixa de ser da conversa e passa a governar o app. A
regra que ela obedece não muda: **esconde aritmética, nunca estado do dado** — o selo
epistêmico sobrevive ao recolhimento.

Segundo consumidor: **Teto do diário**, onde a copy do app já divergiu do motor uma vez.

### D4 — Assinatura 3: o gato marca a voz da Mia

Um glifo, um papel:

- **`NekoMark`** — a marca do app. Só no shell.
- **`MiaAvatar`** — a voz da Mia. Onde ele aparece, a frase é interpretação da copilota, não
  dado da planilha.

O gato é atribuição, como assinatura de autor — nunca mascote performando, que é
anti-referência declarada (`PRODUCT.md`). Consequências: O ano troca `NekoMark` por
`MiaAvatar` na linha da Mia; o gato estampado na face do cartão sai, por não significar nada.

Empty states recebem o gato **apenas quando o vazio é lacuna do método e há o que ensinar
sobre o que fazer** — empty state que ensina a interface, nunca carimbo em todos.

### D5 — Assinatura 4: termômetro e réguas

Toda régua de progresso é o `Meter` do DS (`ui-standards` 15), já disciplinado em quatro
telas. Entra na lista como está: é a régua do método tornada visível.

### D6 — Contrato de motion: um orçamento, governado por token

- **Orçamento único de entrada: ~400ms do início ao fim da sequência**, incluindo o atraso do
  último elemento. Cinco ondas herdaram a frase "coreografia de entrada única por montagem"
  sem o número, e as sequências divergiram até quase um segundo — cinco tempos diferentes são
  o oposto de assinatura de motion.
- **As coreografias usam os tokens `--dur-*`, e ninguém LIGA movimento por media query.**
  Duração hardcoded sob `@media (prefers-reduced-motion: no-preference)` é invisível para o
  atributo `[data-motion]`: o toggle "Animações" das Configurações não desliga o que promete,
  e ligá-lo explicitamente não restaura nada sob movimento reduzido do sistema. O token
  colapsa para 0ms nos dois gatilhos e é restaurado pela escolha explícita do usuário.
- **Indicador em laço é a exceção, e ela tem preço.** Um giro `infinite` não sobrevive à
  duração 0ms — ele trava em vez de parar. Esses vivem pela regra inversa, `animation: none`
  explícito, e por isso todo kill por media query carrega o par `[data-motion="off"]`: sem
  ele, o gatilho do atributo fica de fora e o toggle volta a não alcançar a animação.
- Dinheiro nunca anima; movimento comunica estado, nunca decora.

### D7 — Caixa alta é para abreviação, nunca para rótulo

`text-transform: uppercase` só vale para conjuntos de abreviação convencionalmente
maiúsculos — dias da semana, meses. Nunca para rótulo, olho ou título de seção: caixa alta
apaga a silhueta da palavra e cobra leitura letra a letra num elemento que é contexto, não
conteúdo. Micro-label uppercase é o idioma dos dashboards corporativos, anti-referência
declarada.

Nenhuma string muda: a copy já está em sentence case e é o CSS que a contraria.

### D8 — O checklist se fecha com recorte, não com maquiagem

Sete dos oito achados do diagnóstico fecham com evidência. O oitavo — vazio abaixo da dobra
— sobrevive no Calendário no desktop, e sai como issue contra a tela: preencher aquele vazio
é decidir **o quê** ocupa o espaço, desenho de tela que pertence ao gate da própria onda.
Este ticket julga o que é transversal; consertar layout de uma tela aqui seria o buffer de
qualidade que ele recusa ser.

## Fora de escopo

- O vazio abaixo da dobra do Calendário (issue #282).
- Adoção do recibo em Este mês, O ano, Horizonte e Cartões — onda de adoção posterior, com a
  regra já registrada.
- A prova da cerimônia no Teto do diário (`ProofCard`) é a mesma assinatura construída à mão,
  e entra na mesma onda de adoção. Ela não coexiste com o recibo da estimativa — prova só
  existe com teto escolhido —, então a regra de um recibo por tela se mantém. Converter exige
  rótulo de linha em `ReactNode` (o dela carrega um `InfoPopover`), mudança de API do
  componente que não cabe junto da extração.
- Redesenho de qualquer tela: esta entrega consolida e corrige, não redesenha.

## Critérios de aceite

1. As quatro assinaturas e os dois princípios registrados em `PRODUCT.md` e
   `docs/ui-standards.md`; a fronteira entre "Como funciona?" e "Ver a conta" é regra escrita.
2. `VerdictHero` no DS, consumido por O ano, Horizonte e Cartões; nenhuma faixa de veredito
   tintada com ícone sobrevive nos Cenários.
3. `Receipt` no DS com dois consumidores (conversa e Teto do diário); a prosa que descrevia a
   fórmula do teto foi substituída, não somada; a preferência governa os dois.
4. `MiaAvatar` é o único glifo de voz da Mia; `NekoMark` só no shell; nenhum gato decorativo.
5. Nenhuma folha de estilo do app liga movimento por media query, e todo kill por media query
   carrega o par que o atributo alcança — verificado por teste sobre as telas E o chrome
   compartilhado; nenhuma sequência de entrada excede ~400ms; as cinco specs das ondas
   carregam o número.
6. `text-transform: uppercase` sobrevive apenas no cabeçalho de dias do Calendário.
7. Gates verdes: `npm run check`, baselines visuais regeneradas do zero e inspecionadas,
   React Doctor sem achado novo.
