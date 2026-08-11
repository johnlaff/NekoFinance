# Spec 041 — Cirurgia da regra 1: didática atrás de pergunta nas quatro telas

## Problem Statement

Quem usa o app todos os dias há meses abre Hoje, O ano, Horizonte e Teto do diário e é recebido pela mesma prosa didática fixa de sempre — parágrafos que explicam conceitos já aprendidos, idênticos em toda visita. A auditoria de copy mediu o custo: Hoje carrega ~127 palavras de prosa permanente e O ano ~147, contra um teto de referência de ~10 nas melhores telas do gênero. A dor não é densidade de dados (a mesma auditoria mediu zero blocos de prosa antes do primeiro dado): é **familiaridade** — texto invariável cobrado do leitor todos os dias, para sempre. É exatamente o que a regra 1 do `docs/ui-standards.md` proíbe, e essas quatro telas a violam.

## Solution

Aplicar a regra 1 às quatro telas com um critério operacional único: **texto invariável recolhe para trás do toque; texto que varia com o dado fica**. Nenhuma didática é deletada sem destino — ela muda de lugar (`InfoPopover`) ou morre porque a regra 41 prova que já existe em outro lugar da mesma tela. O herói de cada tela é reescrito na voz que observa: constata o dado do usuário (datado, com valor) e, apenas quando há decisão a tomar, devolve uma pergunta curta — em registro calmo, nunca repreendendo.

Metas verificáveis, medidas pelo método da auditoria (contagem de palavras de prosa permanente visível e de blocos antes do primeiro dado, por captura):

- Hoje: ≤ ~30 palavras fixas (hoje ~127).
- O ano: ≤ ~35 palavras fixas (hoje ~147).
- Zero parágrafo invariável inline nas quatro telas.
- Nenhum popover existente perde conteúdo; didática recolhida permanece integralmente acessível.

## User Stories

1. Como usuário veterano, quero abrir Hoje e ver o veredito e os dados sem reler explicações que já conheço, para decidir o gasto do dia num relance.
2. Como usuário no celular, quero a primeira tela de Hoje inteira dedicada ao conteúdo primário (regra 18), para não rolar até o que vim buscar.
3. Como usuário aprendendo o método, quero tocar "Como funciona?" e ler a explicação completa do conceito, para aprender no momento da dúvida sem pagar o texto todos os dias.
4. Como usuário aprendendo o método, quero tocar um termo sublinhado dentro de uma legenda (ex.: "gasto típico"), para entender aquele termo sem perder o contexto da frase.
5. Como usuário veterano, quero conferir a origem de cada número pelo recibo ("Ver a conta"), para auditar a aritmética sem atravessar prosa.
6. Como usuário, quero que valores, datas e legendas de cálculo continuem sempre visíveis, para que nenhum número fique ambíguo ("R$ 350,00 de quê, contra o quê?").
7. Como usuário num dia tranquilo, quero um herói que apenas observa o meu dado, para não ser interrompido por pergunta retórica.
8. Como usuário diante de um aperto, quero que o herói constate o cenário com data e valor e me devolva a decisão com uma pergunta curta, para agir em vez de me sentir repreendido.
9. Como usuário de O ano abaixo da faixa, quero a pergunta com as duas alavancas do método (soltar menos ou entrar mais), para escolher o caminho sem tom de cobrança.
10. Como usuário do Horizonte em aperto, quero ver o mês e o valor que faltam, com a receita de travessia a um toque, para planejar a passagem pelo buraco.
11. Como usuário do Teto do diário, quero a manchete com o número puro e a explicação do velocímetro a um toque, para nunca ler a mesma didática duas vezes na mesma tela.
12. Como usuário, quero cada dado impresso uma única vez por tela (regra 41), para não ter que conferir se dois textos dizem o mesmo.
13. Como usuário do card da Mia, quero uma observação que muda com o meu mês, para que o card mereça releitura diária.
14. Como usuário de leitor de tela, quero cada gatilho "Como funciona?" nomeando o card a que pertence, para saber qual explicação estou abrindo.
15. Como usuário, quero que nenhuma didática desapareça do app sem destino, para continuar podendo aprender tudo o que o app ensinava antes da cirurgia.
16. Como usuário registrando o primeiro gasto do dia, quero o convite de lançamento apenas quando o dia ainda está sem registro, para que a instrução não vire ruído nos dias já registrados.
17. Como mantenedor, quero a copy travada por asserção de texto (nunca por screenshot), para que mudança de frase jamais passe despercebida por limiar de pixel.
18. Como mantenedor, quero as capturas de referência geradas com dados ricos e realistas, para que a evidência visual mostre as telas como vividas, não vazias.
19. Como leitor da documentação, quero a regra 1 com seu critério operacional registrado e o glossário com os termos novos, para aplicar a mesma fronteira em qualquer tela futura.

## Implementation Decisions

### O critério da fronteira (governa tudo)

Três testes decidem o destino de cada texto, aplicados **por cláusula**, não por frase:

1. **Teste da notação** — se vira notação sem perda, é legenda de cálculo: fica.
2. **Teste da variação** — se muda quando o dado muda, fica; se é idêntico em toda visita, é didática: recolhe.
3. **Teste do leitor veterano** — operandos ele ainda confere; metáfora e explicação, não.

Numa frase mista (esqueleto didático fixo + operando interpolado), a cláusula conceitual recolhe e o operando permanece como legenda curta. É assim que a meta de palavras se mede.

### Perímetro

Somente superfícies de leitura diária. **Ficam intactos**: estados vazios (convite de primeira visita, padrão EmptyState + CTA da regra 3) e o rito do teto (fluxo guiado; o texto ali é o produto).

### Vocabulário novo (entra no glossário e na documentação viva)

- **Selo do veredito**: a linha única de corpo sob a manchete, que muda com o estado do veredito (regra 42 permite exatamente uma). Selos ficam.
- **Legenda de cálculo**: rótulo de operandos do número impresso logo acima. Nunca recolhe (regra 3).

### Regra de gatilho

Didática que explica o bloco inteiro entra atrás de **"Como funciona?"** (padrão canônico já existente, com nome do card em texto só-para-leitor-de-tela). Dúvida sobre um termo dentro de uma legenda que fica vira o **próprio termo tocável**. Nunca os dois gatilhos no mesmo bloco.

### A voz do herói

Observação sobre o dado do usuário, datada e com valor, em registro calmo. **A pergunta curta entra apenas quando há decisão a tomar** (aperto, faixa rompida); estado tranquilo é observação pura. A pergunta devolve a decisão — nunca cobra.

### Copy canônica

Cada linha abaixo vira asserção de texto nos testes. `{…}` marca operando variável.

| Tela      | Estado                     | Copy                                                                                                        |
| --------- | -------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Hoje      | Tranquilo (débito)         | Pode gastar hoje **{R$ 350,00}** — sem nenhum dia no vermelho.                                              |
| Hoje      | Economia como limite ativo | Pode gastar hoje **{R$ 350,00}** — sem tocar na economia do ano.                                            |
| Hoje      | Teto zero                  | O teto de hoje é zero — dia **{15}** o saldo encosta no vermelho. O que dá para mover?                      |
| Hoje      | Card da Mia                | Fechando assim, **{junho}** termina em **{Folga}** — saldo previsto **{R$ 12.340,00}**. + CTA "Ver a conta" |
| O ano     | Dentro/acima da faixa      | Manchete atual (Você guardou **{24%}** do que ganhou.) + selo atual — ficam                                 |
| O ano     | Abaixo da faixa            | Você guardou **{12%}** até aqui. O que aproxima o ano dos 20 — soltar menos ou entrar mais?                 |
| O ano     | Card da Mia                | **{Junho}** guardou pouco — a média do ano segue em **{22%}**.                                              |
| Horizonte | Livre                      | Caminho livre até o fim de **{agosto}**. (atual — fica)                                                     |
| Horizonte | Aperto                     | O caminho aperta em **{setembro}** — faltam **{R$ 1.200,00}**. O que dá para mover antes?                   |
| Teto      | Escolhido (débito)         | Seu dia comporta **{R$ 43,00}**. (manchete pura, corpo morre)                                               |
| Teto      | Escolhido (cartão)         | Seu teto é **{R$ 43,00}** por dia. (manchete pura, corpo morre)                                             |

### Mapa de destinos por tela

**Hoje** (~127 → ~30 palavras fixas):

- A linha didática sob o herói morre pela regra 41: a cláusula conceitual já vive no popover "Como funciona?" do veredito; o operando datado migra para o recibo ("Ver a conta").
- A linha fixa de apresentação da seção da Mia morre seca (meta-comentário de interface, sem pergunta natural que a abrigue).
- A instrução de lançar o gasto vira copy de estado vazio do card (só aparece com o dia sem registro).
- O card da Mia perde as cláusulas fixas e mantém observação variável + CTA. Os operandos que a prosa narrava (ponto mais apertado datado, buraco do futuro) viram linhas do recibo; a próxima entrada morre pela 41 (já é linha de "Próximos movimentos").
- O teto informado morre do herói no modo débito (o denominador da régua do Diário já o imprime) e sobrevive como legenda curta e tocável no modo cartão, onde nenhum outro bloco o imprime. Os estados sem teto mantêm o convite visível uma única vez (regra 3 + regra 4).
- Na legenda do termômetro, a fronteira em R$ fica e a cláusula de origem da régua recolhe para o termo tocável "Termômetro" (verbete do glossário já existente).

**O ano** (~147 → ~35 palavras fixas):

- Manchete + selo do veredito ficam; qualquer cláusula didática extra do corpo recolhe para o popover da faixa (já existe). Abaixo da faixa há dois estados: com economia registrada, o selo é a pergunta das duas alavancas; sem nenhuma, ele mantém o operando da sobra (o percentual já é a manchete) e a leitura da troca certa entra na versão do popover da faixa para o zero por escolha.
- No card de fechamento do ano, as legendas de cálculo ficam; a prosa didática recolhe atrás de "Como funciona?" do próprio card.
- A explicação de entradas/saídas brutas (fluxo de terceiros) recolhe para popover do card de números — didática real, sem duplicata, chave de leitura do card.
- A instrução de tocar num mês morre (affordance se anuncia; acessibilidade via aria).
- A cauda didática do card de renda recolhe para o popover do card.
- No card da Mia, a metade didática fixa morre pela 41 (duplica o popover da faixa); a abertura vira observação variável.
- A legenda de fronteira vivido/previsto fica (rótulo, não didática).

**Horizonte** (zero parágrafo invariável):

- A receita fixa de travessia do aperto recolhe para o popover do buraco do futuro (termo do glossário já usado na tela); a observação variável fica no herói.
- A regra dos 60% de lastro impressa inline morre pela 41 (já vive integralmente nos popovers de gasto típico e do semáforo); ficam as legendas curtas da estrada e da grade.
- A frase de fronteira entre as réguas (ano × caixa) é incorporada ao corpo do popover do semáforo e morre inline.
- A didática do bloco de cenários recolhe atrás de "Como funciona?"; o CTA de testar cenário permanece visível.
- As caudas condicionais ao estado ficam (são o selo do veredito desta tela).

**Teto do diário** (zero parágrafo invariável):

- Estados escolhido/débito e escolhido/cartão perdem toda a prosa do corpo pela regra 41 (velocímetro e modo cartão já vivem nos popovers da própria tela); manchete pura.
- Estado estimativa mantém selo + convite curto ao rito (estado transitório, com CTA).
- No cartão de idade da cerimônia, a regra dos três meses morre (já vive no popover da cerimônia); fica a legenda com operando (data + situação da recalibragem).
- Na prova do número, a cauda do arredondamento encurta para legenda de notação junto à fórmula; a nota da planilha segue como está (já atrás de disclosure fechado).

### Ordem de execução

0. Regerar as capturas de referência com fixture rica (higiene de evidência — ticket avulso, primeiro).
1. Hoje.
2. Teto do diário (par de Hoje: compartilham os popovers das duas réguas).
3. O ano.
4. Horizonte (par de O ano: herda a fronteira de réguas no popover do semáforo).

### Paper trail (no primeiro PR da frente)

- ADR curto: "didática recolhe, dado fica" — diagnóstico (familiaridade, não densidade), o critério da cláusula e o escopo contido.
- `docs/ui-standards.md`: os três testes da fronteira anexados à regra 1 como critério operacional.
- `CONTEXT.md`: entradas para **selo do veredito** e **legenda de cálculo**.

## Testing Decisions

- **Um bom teste desta frente testa comportamento externo**: o texto que o usuário vê (queries de texto/aria), nunca a estrutura interna dos componentes. Cada linha da copy canônica vira asserção de presença; cada bloco morto vira asserção de ausência. O RED do TDD é a asserção da copy nova falhando contra a tela atual.
- **Seams (os três existentes; nenhum novo)**:
  1. Render do componente de tela (vitest + testing-library) — seam principal de copy; os quatro arquivos de teste de tela já travam copy hoje e são o prior art direto.
  2. Suíte visual Playwright — copy no nível da página via snapshot de aria versionado; screenshot apenas para layout/cor/oclusão (regra 38: limiar absoluto de pixels; regeneração de baseline do zero, rodando duas vezes). O ticket 0 usa este seam com o fixture enriquecido.
  3. View-models puros — unit test somente onde uma frase derivada muda de forma (observação variável da Mia, herói com pergunta condicional ao estado).
- Popovers novos são dados (`{title, body}`) renderizados pelo componente de disclosure existente — testados pelo seam 1.
- Gate por tela entregue: verificação visual da primeira tela mobile (390×844, regra 18) com inspeção das capturas, e auditoria de qualidade de interface antes de considerar a tela pronta.

## Out of Scope

- Estados vazios e o rito do teto (perímetro: só leitura diária).
- Porta de onboarding e qualquer heurística de decaimento sobre texto.
- Redesenho estrutural de navegação ou dos layouts em bento.
- Qualquer mudança de direção estética (a variante vigente é a clean tipográfica; regras 24/35/37 do ui-standards intactas).
- "Convites que se aposentam" (memória local por conceito) — frente própria futura.
- Glossário navegável indexado por termo — spec própria, pós-cirurgia.
- Qualquer mudança nas regras do método, fórmulas ou motor de cálculo: a cirurgia move texto, não lógica de domínio.

## Further Notes

- A medição das metas usa o mesmo método da auditoria que as estabeleceu: contagem de palavras de prosa permanente e de blocos antes do primeiro dado, por captura de tela — o que permite verificar o antes/depois com evidência.
- Dinheiro nunca anima; cores de status independentes do acento; dado ausente nunca vira zero — invariantes do app que a cirurgia não toca, listados aqui porque os heróis reescritos passam perto deles.
- As quatro telas seguem o mesmo esqueleto retórico do design system (manchete → selo → dados → didática a um toque); a cirurgia converge as quatro para o padrão que as telas já corretas praticam — imitar, não inventar.
