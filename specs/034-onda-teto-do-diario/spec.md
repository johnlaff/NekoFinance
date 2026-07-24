# Spec 034 — Onda Teto do diário: o registro de uma decisão com prova

## Contexto

A tela do teto era um **editor**: um card único com controle segmentado (por itens ×
valor direto), lista de inputs, divisor e "Salvar teto", precedido por um banner quando
a planilha propunha uma cerimônia. Ela pedia trabalho a cada visita, e o teto — o número
que o dia inteiro respeita — aparecia como resultado de formulário, não como veredito.

A direção redefine a leitura: **o teto é o registro de uma decisão com prova**. A
cerimônia que o produz é rara (a régua do método recalibra de três em três meses); no
resto do tempo a tela é prova e referência. Por isso a tela abre pelo número decidido,
segue pela prova que o produziu (os itens do mês variável, a fórmula e a nota original da
planilha), depois pela idade da cerimônia, e fecha explicando como o dia lê esse teto. A
edição deixa de ser o estado permanente da tela e vira um **rito guiado de três batidas**,
na própria superfície — nunca modal, nunca wizard de tela cheia.

A matemática não muda: `total mensal ÷ divisor de dias`, **arredondado para cima** (teto é
teto), a mesma regra do núcleo Rust. A composição — estado da manchete, idade da
cerimônia, guarda do teto que baixa, validação do divisor — vive num **view-model puro**
(`src/screens/tetoView.ts`) com TDD, no padrão das telas irmãs (`hojeView`, `anoView`,
`tagsView`).

O backend ganha **proveniência**, não regra: a nota da célula que documenta a cerimônia
passa a ser persistida (hoje só o hash dela sobrevive ao import), para que a citação
literal na tela seja reprodução, não reconstrução.

## Estrutura da tela (ordem do DOM = ordem de leitura)

1. **Veredito** — "Seu teto é R$ 40,33 por dia." com o rótulo da procedência ("Teto do
   diário · Estipulado em setembro de 2025"), uma frase que declara o recorte do modo
   detectado e o `ModeChip`. Seis estados epistêmicos (abaixo).
2. **A prova do número** (card) — as linhas da cerimônia (categoria · dinheiro livre ·
   R$/mês), a fórmula em três linhas (Total do mês variável ÷ Dividido por = Teto por
   dia), a nota do arredondamento e o disclosure **"Ver a nota original da planilha"**
   com a citação em mono. Só existe quando há cerimônia itemizada.
3. **A idade da cerimônia** (card) — "A cerimônia fez dez meses" + a cadência do método,
   em tom neutro (**zero cor de status**: o método não julga o teto) + CTA "Recalibrar o
   teto".
4. **Como o dia lê o teto** (card) — o fato do modo corrente em uma frase; a didática de
   como a leitura muda vive atrás do termo, no popover.

No mobile a leitura é uma coluna só, na ordem do DOM. No desktop, a narrativa curta em
coluna única deixava a metade direita da janela vazia — a régua de ambiente ganha do
desenho aprovado: o veredito ocupa largura cheia (com a linha de leitura travada no
texto) e os cards descem para um **bento de duas colunas independentes** a partir de
900px, que se dissolvem em `display: contents` no mobile. Durante o rito, a coluna
direita mantém a prova vigente à vista: é o "antes" que o aceite substitui.

O retorno para a Hoje é do shell (sidebar no desktop, dock no mobile) — a tela não
duplica navegação.

## O veredito (seis estados epistêmicos)

O `tetoView` seleciona o estado a partir de três fontes: o orçamento ativo
(`get_daily_budget`), a proposta pendente (`get_ceiling_proposal`) e a leitura do dia
(`get_dashboard_summary`: `daily_ceiling_source`, `daily_budget`, `spending_mode`).

1. **Escolhido · modo cartão** (estado real dos dados de referência) — "Seu teto é R$
   {v} por dia." + "O dia é medido pelas faturas enquanto você vive no crédito — e o
   teto fica de guarda: a régua que você consulta antes de qualquer gasto livre."
2. **Escolhido · modo débito** — "Seu dia comporta R$ {v}." + a frase do velocímetro
   (ver "Fidelidade ao motor").
3. **Proposta da planilha** — "Sua planilha propõe R$ {v} por dia." + a cerimônia
   anotada (total ÷ divisor, mês da nota) + "Usar este teto" / "Agora não". Nada é
   gravado sem aceite.
4. **Estimativa** — "Cerca de R$ {v} por dia, pelo seu histórico." com o selo
   `EstimateMark`: é a média do Diário dos meses com registro, não um teto escolhido.
   CTA "Estipular o teto".
5. **Sem registro** — "Você ainda não tem um teto." + como o teto nasce, em uma frase +
   CTA "Estipular o teto" e o escape "Já sei meu teto".
6. **Carregando** — `EmptyState` variante esqueleto no lugar do veredito (nunca spinner
   sobreposto, nunca `R$ 0,00` fabricado).

A cor de status do método **não aparece** em nenhum deles: o método não julga o teto, e a
tela prova a regra da separação mantendo `--ok`/`--warn` fora do consumo.

## O rito da recalibração (três batidas)

Aberto pelo CTA (recalibrar ou estipular), na própria superfície: o veredito permanece no
topo, e os cards de prova/idade/leitura dão lugar à batida corrente. Nunca modal.

1. **Batida 1 — o mês variável.** "O que o seu mês variável comporta?" Linhas
   editáveis (nome + valor mensal + remover), "Adicionar categoria", total corrido
   anunciado em `aria-live`, escape "Prefiro digitar o valor direto". CTA "Definir os
   dias".
2. **Batida 2 — o divisor.** "Por quantos dias dividir o total?" O palco da divisão
   (total ÷ campo × "dias"), a dica da régua fixa, e o **erro calmo**: divisor vazio ou
   ≤ 0 desabilita o avanço com microcopy serena inline. CTA "Ver o teto novo".
3. **Batida 3 — o aceite.** "O seu teto novo está pronto." Antes → depois com
   prospectividade explícita ("Vale daqui para frente — os dias já vividos não mudam"),
   a fórmula que o produziu e a procedência — "Calculado agora, com os itens que você
   revisou", porque é o que o app de fato testemunha: os itens, não o extrato. CTA
   "Usar este teto".

**A guarda do "vença o dia"** intercepta a batida 3 quando o teto novo é **menor** que o
atual: descreve a consequência (baixar por esperança pinta a planilha de verde, o extrato
segue o mesmo) e libera a escolha — "Baixar assim mesmo" existe ao lado de "Manter R$ {v}
por dia". A guarda ensina e libera; nunca tranca, nunca rotula a intenção.

**A cerimônia guiada** substitui a batida 1 quando não há teto nenhum: cinco perguntas na
voz da casa, uma por vez (comida, transporte, saúde, lazer, compras), com o escape "Já sei
meu teto — digitar direto". No fim, o mesmo divisor e o mesmo aceite.

**O valor direto** é um caminho, não um modo permanente: o escape — sempre com a mesma
frase, "Já sei meu teto", nas três superfícies que o oferecem — leva a um campo único
(teto por dia) com o mesmo aceite. Ele grava sem cerimônia — e a tela, a partir daí,
mostra o veredito sem card de prova (não há o que provar).

## Proveniência (o que muda no backend)

A citação da nota é **reprodução**, então a nota precisa sobreviver ao import:

- `ceiling_proposal.raw_note` — a nota crua da célula, gravada junto com a proposta.
- `daily_budget.source_note` — a nota que sustenta o orçamento ativo, propagada no
  aceite da proposta; **limpa** quando o teto é estipulado no rito (a partir daí a prova
  é a cerimônia do app, não a nota da planilha).
- `daily_budget.ceremony_month` (`YYYY-MM`) — quando a cerimônia foi feita: o mês da
  nota, no aceite da proposta; o mês corrente, no rito. Backfill dos registros
  existentes com o mês do `start_date`.

Os DTOs expõem `source_note`/`ceremony_month` (`DailyBudget`) e `raw_note`
(`CeilingProposal`). Nenhuma regra de cálculo muda; o motor do teto (`effective_daily_ceiling`,
`daily_ceiling_reading`) fica intocado.

## Divergências entre o desenho e as réguas do repositório

Três pontos do protótipo aprovado mudam na implementação, cada um por uma regra escrita:

1. **A frase do "menor de dois limites" no modo débito.** O protótipo diz que o
   velocímetro "orienta pelo menor de dois limites: o teto e o que o caixa aguenta". O
   motor não computa esse `min`: `safe_to_spend_today` é o mais apertado entre **caixa** e
   **economia do ano** — o teto não entra na conta. A tela diz a verdade do motor: o
   velocímetro mede o Diário lançado contra o teto; o "Dá para gastar hoje" é a outra
   régua, e no dia a dia vale o mais apertado dos dois (a mesma língua da Hoje).
   `ui-standards` §6.
2. **"Fazer a cerimônia" vira "Estipular o teto".** A Hoje já convida com "Estipular o
   teto" no estado sem teto; duas frases para o mesmo ato quebram a invitação única.
   `ui-standards` §4.
3. **A didática de "Como o dia lê o teto" recua para o popover.** O protótipo traz um
   parágrafo permanente; o que varia (qual régua o dia está medindo agora) fica inline, e
   a explicação estável (o que acontece ao migrar de modo) vive atrás do termo.
   `ui-standards` §1.

## Motion

Entrada de batida: **fade + 8 px, 420 ms**, curva `cubic-bezier(.2, 0, 0, 1)`, uma
superfície por gesto — e o foco acompanha (o título da batida recebe foco programático,
`tabindex="-1"`). A superfície do aceite se materializa com o mesmo par; **o dinheiro
nunca anima**: entra pronto, não conta, não rola, não pisca. Com `prefers-reduced-motion`
tudo vira troca instantânea, e nada depende do movimento para ser compreendido.

## Acessibilidade

- Cada batida é uma região nomeada (`aria-labelledby` no título) com o passo declarado
  ("Batida 2 de 3") — os pontos decorativos são `aria-hidden`.
- O total corrido do mês variável é `aria-live="polite"`: remover uma categoria anuncia o
  novo total. O desfazer é inline, nunca modal.
- O antes → depois tem equivalente textual no `aria-label` do grupo ("Teto sai de R$
  40,33 para R$ 43,55 por dia, válido daqui para frente"); a linha visual é
  `aria-hidden`.
- Erro de divisor: `role="alert"` inline, ligado ao campo por `aria-describedby`, com o
  avanço desabilitado enquanto durar.
- Alvos de toque ≥ 44 px (o glifo pode ser menor que a área); foco visível em toda
  superfície interativa; a citação em mono preserva quebras (`white-space: pre-wrap`) e é
  lida como bloco de código.

## Fidelidade ao método (traço às fontes)

- **A cerimônia** é `soma dos itens variáveis ÷ dias`, com arredondamento **para cima** —
  a aula do diário e a nota real da planilha (`R$ 1250,00 / 31 Dias = R$ 40,33`) fecham no
  mesmo centavo que `Math.ceil`.
- **O divisor fixo em 31** é a prática registrada na planilha o ano inteiro; a tela não
  força o calendário do mês, só oferece o número anotado.
- **A cadência de três em três meses** é a régua do método para refazer a cerimônia — a
  tela informa a idade e convida, nunca cobra.
- **As cinco perguntas** da cerimônia guiada (comida, transporte, saúde, lazer, compras)
  são o roteiro de onboarding do método.
- **O modo detectado** é o do motor de estados de dado ausente (histerese), não uma
  escolha editorial da tela — e não existe override manual.

## O que morre

- O `SegmentedControl` "Por itens (cerimônia) × Valor direto" como estado permanente: o
  valor direto vira escape dentro do rito.
- O banner de proposta acima do editor: a proposta passa a ser um **estado do veredito**.
- O botão "Remover teto" solto no rodapé do editor: sem invitação concorrente na tela de
  leitura (remover volta pelo rito, gravando teto zero via "Prefiro digitar o valor
  direto" com campo vazio não é caminho — a remoção sai do escopo desta onda; ver Fora de
  escopo).
- "Salvar teto" como CTA genérico: cada batida tem o seu verbo.

## Fora de escopo

- **Remover o teto** pela tela: o caminho existia como botão solto e sai com o editor.
  Volta como decisão própria (o método não prevê "desestipular"; o que existe é
  recalibrar).
- **Colar em lote** os itens da planilha e atalhos de teclado por batida — eficiência de
  power user, avaliada depois do uso real.
- **Nota didática ligando o salto do teto ao gate da economia** (subir o diário mantendo a
  proporção): depende de leitura anual dentro desta tela, que a onda não abre.
- **Override manual do modo** — decisão selada: a detecção é automática pura.

## Aceitação

- Os seis estados do veredito renderizam com os dados que os produzem, e nenhum fabrica
  `R$ 0,00`.
- A prova do número reproduz a nota original quando ela existe, e o disclosure não
  aparece quando não existe.
- O rito percorre as três batidas na superfície, com foco acompanhando, erro calmo no
  divisor e guarda quando o teto baixa — e grava exatamente o número exibido no aceite.
- `npm run check` verde, baselines visuais regeneradas do zero (desktop + mobile),
  React Doctor sem novas violações, auditoria e crítica de UI sem achados abertos.
