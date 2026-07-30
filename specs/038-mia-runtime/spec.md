# Spec 038 — Mia v1: runtime, fachada de 14 ferramentas, bakeoff e consentimento

## Problem Statement

A conversa tem a forma final e não tem substância.

A tela publica respostas no formato que o método exige — recibo auditável com operandos, operador e resultado; selo epistêmico no número derivado; quatro recusas honestas; proveniência no pé da bolha. Só que os números nascem de seis contas determinísticas escritas no frontend, sobre os DTOs que a tela já tinha. Qualquer pergunta fora dessas seis recebe "a conversa ainda não está ligada".

Para a pessoa, isso significa:

- Perguntar em linguagem aberta não funciona. Só as seis perguntas do repertório respondem, e descobrir quais são exige tentativa e erro.
- Nada que dependa de recorte próprio é alcançável: "quanto gastei com o cartão adicional em maio", "quais compras parceladas ainda têm parcelas em aberto", "meu Economizado% deste ano bate a faixa?".
- A Mia não ensina. O método vive em capítulos curados no pack local, e nenhuma pergunta chega neles.
- Registrar um lançamento pela conversa cai na recusa de capacidade, que nomeia o gesto global de Lançar — a conversa observa, nunca ajuda a agir.
- A conversa não persiste: fechar o app apaga tudo, e não existe o gesto de apagar (não há o que apagar).

Para quem mantém o app, significa que a maior promessa do produto está descrita, decidida em três tickets e não construída — e que a decisão de qual modelo usar segue sendo aposta, não medição.

## Solution

Ligar a conversa: um loop de agente in-process no backend, com capacidades fechadas e saída verificável.

A pessoa pergunta o que quiser. Um modelo de linguagem, rodando por um provedor único com privacidade contratada, escolhe entre 14 ferramentas de leitura que atravessam tudo o que a interface mostra. **O modelo nunca calcula**: todo número material — inclusive deltas, percentuais e comparações — vem pronto da ferramenta, e uma resposta que cite número sem origem em fato retornado na mesma rodada é descartada e regenerada. Quando a pergunta não tem resposta, a recusa diz qual das quatro portas fechou e oferece a saída concreta.

Registrar pela conversa existe como **proposta**: o modelo monta o lançamento canônico e o entrega assinado; a aprovação acontece fora do loop, pelo mesmo caminho interno do formulário. Texto no chat nunca aprova nada.

A conversa persiste e é apagável de verdade. Cada rodada declara provedor efetivo, modelo e custo. O consentimento diz a verdade sobre o que sai da máquina, e o backend recusa qualquer rodada sem ele.

Sem chave ou sem consentimento, as seis contas locais continuam respondendo offline — a recusa "ainda não está ligada" é literalmente verdadeira e traz o caminho de ligar.

Antes de qualquer disso ir ao ar, um bakeoff mede cinco modelos candidatos mais um teto de referência sobre a suíte de evals própria, e é o resultado — não a intuição — que escolhe o default.

## User Stories

### Perguntar e receber diagnóstico

1. Como pessoa, quero perguntar em linguagem aberta sobre meus números, para que eu não precise descobrir por tentativa e erro quais perguntas a conversa aceita.
2. Como pessoa, quero que toda resposta com número traga o recibo com os operandos, para que eu possa conferir de onde o número veio sem confiar na frase.
3. Como pessoa, quero que a conversa alcance qualquer dado que a interface mostra, para que eu não tenha de aprender qual tela guarda qual número.
4. Como pessoa, quero perguntar sobre um mês específico e comparar com outro, para que eu veja movimento em vez de fotografia.
5. Como pessoa, quero perguntar sobre o ano corrente e o anterior, para que eu julgue o Economizado% na régua anual que o método usa.
6. Como pessoa, quero filtrar lançamentos por período, valor, conta, tag, pessoa responsável e forma de pagamento, para que eu responda perguntas de recorte próprio.
7. Como pessoa, quero perguntar quanto ainda posso gastar hoje, para que eu tenha o mesmo veredito da tela Hoje sem trocar de tela.
8. Como pessoa, quero perguntar sobre as faturas em aberto e o próximo vencimento, para que o balde do cartão fique visível na conversa.
9. Como pessoa, quero perguntar sobre séries e parcelas em aberto, para que eu saiba o que já está comprometido nos próximos ciclos.
10. Como pessoa, quero perguntar se tem buraco no caixa à frente, para que eu antecipe aperto em vez de descobrir no dia.
11. Como pessoa, quero perguntar sobre a reserva em meses de custo de vida, para que eu saiba onde estou na fundação do método.
12. Como pessoa, quero simular uma hipótese sem persistir nada, para que eu teste uma decisão antes de assumi-la.
13. Como pessoa, quero que a resposta cite o estado epistêmico do número (veredito, estimativa, zero legítimo, sem registro), para que eu nunca confunda dado vivido com dado derivado.
14. Como pessoa, quero que a conversa respeite o modo de gasto detectado, para que a resposta não me cobre um Diário zerado por escolha.

### Recusar com honestidade

15. Como pessoa, quero que a recusa diga qual porta fechou — sem dado, capacidade não suportada, pergunta ambígua, conversa não ligada —, para que eu saiba se o problema é meu, do app ou do dado.
16. Como pessoa, quero que a recusa por dado ausente ofereça o caminho de importar ou lançar, para que a conversa termine com um gesto possível.
17. Como pessoa, quero que uma pergunta ambígua vire pergunta de esclarecimento, para que a conversa nunca suponha valor, data, tipo ou conta.
18. Como pessoa, quero que a pergunta por gasto por categoria me ensine que a tag é interruptor de régua, para que a recusa vire aula em vez de parede.
19. Como pessoa, quero que a conversa recuse pedir análise que ela não pode calcular, para que ela nunca invente número plausível.

### Registrar por proposta

20. Como pessoa, quero descrever um lançamento em texto e receber uma proposta estruturada, para que registrar pela conversa seja possível sem que ela escreva sozinha no meu histórico.
21. Como pessoa, quero aprovar a proposta com um gesto explícito na interface, para que nada entre no meu histórico por concordância escrita no chat.
22. Como pessoa, quero editar a proposta antes de aprovar, para que corrigir um campo não me obrigue a recomeçar a conversa.
23. Como pessoa, quero que editar a proposta invalide a aprovação anterior, para que eu nunca aprove um valor e registre outro.
24. Como pessoa, quero que a proposta expire se os dados mudarem embaixo dela, para que a aprovação valide o mundo de agora, não o de dez minutos atrás.
25. Como pessoa, quero que a proposta aceite só entrada ou despesa avulsa, para que recorrência, parcelamento, split e transferência continuem nos formulários que os tratam direito.
26. Como pessoa, quero que aprovar crie o lançamento local sem tocar a planilha, para que a convergência siga pelo fluxo de write-back que já exige diff e aprovação.

### Confiar: transparência, privacidade e consentimento

27. Como pessoa, quero ler, antes de ligar a conversa, exatamente o que sai da máquina, para que eu decida com o fato na mão em vez de com uma promessa vaga.
28. Como pessoa, quero que o consentimento nomeie os dois processadores envolvidos, para que "a nuvem" tenha nome.
29. Como pessoa, quero que o texto declare que lançamentos completos, com descrições e notas, podem ser enviados, para que o consentimento não minta por omissão.
30. Como pessoa, quero que o backend recuse rodar sem consentimento registrado, para que a garantia não dependa de a tela esconder um botão.
31. Como pessoa, quero ver provedor efetivo, modelo e custo de cada rodada, para que o gasto e o destino do dado sejam verificáveis a cada pergunta.
32. Como pessoa, quero que minha chave viva no cofre do sistema operacional, para que ela nunca apareça em log, evento, banco ou tela.
33. Como pessoa, quero que a conversa fale só com os domínios permitidos, para que nenhuma resposta consiga desviar meus dados para outro destino.
34. Como pessoa, quero apagar a conversa de verdade, para que o histórico não sobreviva ao meu gesto de apagar.
35. Como pessoa, quero que apagar a conversa preserve a proveniência dos lançamentos que aprovei, para que meu histórico financeiro não perca a origem.
36. Como pessoa, quero um checklist no onboarding para desligar os opt-ins de conteúdo da minha conta no provedor, para que a configuração que a requisição não controla também fique fechada.

### Ler a conta no meu ritmo

37. Como pessoa, quero uma chave que recolha o recibo em toda resposta, para que quem já confia na conta não pague a prova inteira toda vez.
38. Como pessoa, quero que a chave venha ligada por padrão, para que a prova seja o estado natural e o recolhimento seja escolha consciente.
39. Como pessoa, quero abrir a conta recolhida ali mesmo, para que conferir um número não me tire da conversa.
40. Como pessoa, quero que o selo de estimativa sobreviva ao recolhimento, para que um número derivado nunca se pareça com um número vivido.

### Continuar funcionando sem rede

41. Como pessoa, quero que as seis contas locais continuem respondendo sem chave e sem consentimento, para que o app siga útil offline e de graça.
42. Como pessoa, quero que a recusa "ainda não está ligada" ofereça o caminho de ligar, para que a limitação venha com a saída.
43. Como pessoa, quero que a resposta local e a resposta do runtime cheguem no mesmo número, para que o meu veredito não dependa de eu estar conectada.

### Aprender o método

44. Como pessoa, quero perguntar o que um termo do método significa, para que eu aprenda no lugar onde a dúvida nasceu.
45. Como pessoa, quero pedir aprofundamento em um tópico do método, para que a conversa entregue o capítulo, não uma frase de dicionário.
46. Como pessoa, quero que a explicação do método venha marcada como explicação, para que ela nunca se disfarce de cálculo sobre os meus números.

### Manter o app

47. Como quem mantém o app, quero um bakeoff que meça os candidatos na suíte de evals própria, para que o modelo default seja escolha medida e não aposta.
48. Como quem mantém o app, quero uma trava dupla de gasto no bakeoff, para que um erro de laço não vire fatura.
49. Como quem mantém o app, quero que o bakeoff sirva de canary do caminho estrito com ferramentas, para que a primeira falha de compatibilidade apareça em bancada e não em produção.
50. Como quem mantém o app, quero que o adapter verifique o endpoint pinado antes de confiar nele, para que mudança externa vire erro diagnosticável em vez de falha silenciosa.
51. Como quem mantém o app, quero um catálogo de evals versionado e um runner local, para que qualquer mudança de fachada, prompt ou modelo seja reavaliada pelo mesmo critério.
52. Como quem mantém o app, quero um manifesto de paridade entre superfícies da interface e ferramentas exercitado em teste, para que tela nova sem ferramenta correspondente quebre o teste.
53. Como quem mantém o app, quero rastro técnico com retenção limitada, para que depurar uma rodada seja possível sem acumular histórico indefinidamente.
54. Como quem mantém o app, quero que o gate de evals seja pré-requisito de ligar, para que a exigência de avaliação antes de ações de agente seja cumprida na prática.

## Implementation Decisions

### 1. Runtime — loop à mão, in-process

Módulo `mia`, com o loop como máquina de estados pequena. Sem framework de agente.

- `ProviderAdapter` é um trait com domínio interno mínimo: `TextDelta`, `ToolCallComplete`, `Usage`, `FinishReason`, `Error`. Cada adapter preserva o transcript nativo do provedor e traduz só na fronteira.
- Ferramentas despacham como chamadas de função nos módulos de domínio já testados.
- Histórico é do app, não do provedor: a janela da conversa corrente é reenviada a cada rodada, com teto de tokens e aviso honesto quando a conversa fica longa. Sem sumarização.
- O prefixo estável (núcleo do método + contexto do app) é montado primeiro justamente para o desconto de cache do provedor incidir sobre ele.

**As 12 invariantes** que o loop honra, cada uma com teste próprio:

1. A chave vive só no cofre do sistema — nunca serializada para frontend, eventos, logs ou banco.
2. Egress restrito à allowlist, com redirect bloqueado.
3. Todo tool call é validado localmente antes de executar; validação falha fecha (fail-closed).
4. Chamadas paralelas de ferramenta desligadas.
5. Teto de turnos por rodada.
6. Teto de chamadas de ferramenta por rodada.
7. Teto de custo por rodada.
8. Teto de tempo por rodada.
9. Cancelamento propaga até a conexão HTTP.
10. Argumento parcial nunca executa.
11. Retry diferencia falha pré-resposta, falha no meio do stream, limite de taxa e falha definitiva.
12. Número na resposta sem origem em fato retornado na mesma rodada: descarta e regenera.

### 2. Fachada — dispatch único sobre os helpers puros

Uma porta de entrada (`dispatch(pool, call) -> envelope`) para as 14 ferramentas, chamando os mesmos helpers puros que os comandos Tauri já chamam. Sem camada de serviço nova, sem espelhar a interface 1:1, e sem mudar a superfície nem o comportamento de comando existente.

A régua vive numa função só. Quando a ferramenta precisa de uma leitura que hoje está embutida no corpo de um comando, a leitura é extraída para helper compartilhado e o comando passa a chamá-lo — comportamento preservado, provado pelos testes que já cercam o comando. Copiar a régua para dentro da fachada é o que não se faz: duas implementações divergem, e o número da conversa deixa de bater com o da tela.

As ferramentas: `get_financial_snapshot` · `get_month_analysis` · `get_year_analysis` · `get_cashflow_calendar` · `search_transactions` · `get_tags` · `get_commitments` · `get_forecast` · `simulate_scenario` · `get_accounts_and_net_worth` · `get_budget_settings` · `get_data_status` · `get_method_guidance` · `propose_transaction`.

Regras transversais do envelope:

- `meta` comum herdado por toda resposta: moeda, timezone, período, `as_of` e revisão dos dados.
- Dinheiro como decimal exato em centavos, nunca float.
- Cursor opaco para paginação — nunca número de página.
- `range` sempre com datas explícitas; `sort` de vocabulário controlado.
- Agregado cobre o filtro inteiro, não a página.
- Defaults enxutos, expansão por `include` — todo dado alcançável, nem todo dado retornado por padrão.
- Deltas e percentuais pré-calculados pela ferramenta.
- Erros acionáveis: dizem o que fazer, não só o que falhou.
- Descrição de cada ferramenta declara "use para" e "não use para".
- Teto de linhas por chamada.
- Resultados de ferramenta são delimitados como dados não confiáveis no prompt.

`propose_transaction` é read-only como as demais: valida, normaliza e devolve o payload canônico com hash amarrado ao schema e à revisão dos dados, com validade. A aprovação roda fora do loop e revalida tudo antes de gravar.

**Manifesto de paridade**: uma tabela versionada superfície da interface → ferramenta que a alcança, exercitada em teste. Superfície sem ferramenta correspondente quebra a suíte.

### 3. Provedor — OpenRouter único, gates fail-closed no código

Os gates são contrato, não configuração: `data_collection: "deny"` · `only` com o endpoint pinado · `allow_fallbacks: false` · `require_parameters: true` · cache de resposta da borda desligado (o cache de prompt do provedor, que é a economia real do loop, fica intacto) · modelo pinado, nunca roteamento automático. Beta header é propriedade do pin e só sai quando o pin o declara.

**Retenção é propriedade do pin.** O padrão é retenção zero: o pin envia `provider.zdr: true` e a prova é a presença no catálogo de retenção zero do provedor. Um pin pode optar por fora de forma DELIBERADA (`Retention::ProviderPolicy`, decisão do dono registrada no próprio pin) quando o desconto do endpoint compensa trocar a prova do catálogo pela política declarada do operador (treino desligado, retenção de log limitada); esse pin não envia `zdr`, e todos os demais gates permanecem. A troca nunca é automática nem silenciosa — é edição manual da matriz, visível no diff.

**Pins são por endpoint, não por provedor** — a matriz que decide é endpoint × parâmetro suportado. Para modelo de peso aberto, o endpoint carrega também a precisão servida: a tag nomeia a quantização, e ela é parte da identidade do candidato — outro endpoint com outra precisão é outro candidato. Só entra precisão que o catálogo declara; `unknown` não identifica o que se mede.

| Modelo                 | Endpoint pinado | Retenção                    | Papel                                             |
| ---------------------- | --------------- | --------------------------- | ------------------------------------------------- |
| `openai/gpt-5.6-terra` | `openai`        | opt-out (política OpenAI)   | default provisório (a corrida promove ou rebaixa) |
| `openai/gpt-5.6-luna`  | `openai`        | opt-out (política OpenAI)   | candidato                                         |
| `openai/gpt-5.6-sol`   | `azure`         | zero (catálogo do provedor) | teto de referência (fase 1)                       |

**Verificação de drift**: antes de confiar no pin, o adapter consulta o catálogo que prova o caminho de retenção dele — o de retenção zero para pin em `Retention::Zero`, o catálogo geral de endpoints do modelo para pin em opt-out — e afirma três coisas: o endpoint existe, pertence ao catálogo que prova o pin, e anuncia os parâmetros que a rodada envia (`tools`, `structured_outputs`, `reasoning` e o teto de saída com o nome que o pin declara). Roda como teste do adapter (contra fixture gravada) e como passo do canary do bakeoff (ao vivo).

### 4. Streaming — eventos tipados, resposta atômica

A interface recebe eventos por canal tipado: `run_started` · `tool_started` · `tool_finished` · `proposal_ready` · `answer_ready` · `usage` · `error` · `run_finished`. Nunca texto token a token: a invariante 12 é incompatível com token já mostrado na tela. O loop consome o stream do provedor internamente para despachar ferramenta cedo, propagar cancelamento e capturar custo; a resposta financeira publica atômica depois de validada.

O evento `usage` carrega custo, provedor efetivo, modelo e tentativas — é a fonte da linha de transparência por rodada.

### 5. Persistência — três stores, uma cascata

Migrações seguindo a convenção de nome datada do repo:

- **Transcript visível** (`mia_conversation` / `mia_message`): persiste entre sessões, apagável de verdade.
- **Trace técnico** (por rodada: chamadas, ferramentas, tokens, custo, provedor, erros, tentativas): retenção de 30 dias com purga automática. Nunca contém a chave.
- **Ledger de propostas** (proposta + hash + decisão + identificador do lançamento criado): durável, nunca purga automaticamente.

Cascata: apagar a conversa apaga transcript e trace; o ledger sobrevive, porque a proveniência de um lançamento aprovado é parte do histórico financeiro, não da conversa.

### 6. Conhecimento — três camadas sobre o pack curado

- **(a)** Núcleo do método no prompt de sistema, montado a partir do pack local curado, somado ao contexto do app e à estrutura da planilha real. Forma agnóstica: "o método", nunca origem nominal.
- **(b)** `get_method_guidance(topic)` serve capítulos completos do pack por tópico, com enum controlado de 11 tópicos. Sem RAG vetorial.
- **(c)** Gate de anonimização mecânico: o conteúdo servido passa pela varredura de privacidade do repo mais a deny-list local antes de qualquer entrega.

Resposta que vem da camada de método carrega proveniência de explicação — nunca se apresenta como cálculo sobre os números da pessoa.

### 7. Consentimento — registrado no banco, verificado no backend

O consentimento vive como registro durável no armazenamento de configurações do app (não em armazenamento do webview), porque quem precisa lê-lo é o loop: **sem registro de consentimento, o backend recusa a rodada**, independentemente do que a interface mostre. A chave vive no cofre do sistema, em serviço próprio.

O texto do consentimento nomeia os dois processadores envolvidos, declara acesso total de leitura (incluindo descrições e notas), e traz o checklist dos dois opt-ins de conteúdo da conta do provedor que a requisição não controla.

As defesas de injeção são estruturais, não censura de dado: ferramentas 100% read-only, aprovação humana fora do loop, ausência de web e de servidores externos de ferramenta (sem canal de exfiltração), resultados delimitados como não confiáveis, teto de linhas, e casos de injeção na suíte de evals.

### 8. Interface

- **Chave "mostrar a conta"** em Configurações › Conversa, ligada por padrão. Desligada, a resposta imprime resultado e proveniência e oferece abrir a conta no lugar. Preferência de superfície, no mesmo padrão das demais preferências de aparência.
- **O selo epistêmico sobe** da linha do recibo para a resposta. A linha mantém o selo dela para o caso de um operando estimado entre operandos vividos. Regra: a chave esconde aritmética, nunca estado do dado.
- **Piso offline**: sem chave ou sem consentimento, as seis contas locais seguem respondendo; a recusa "ainda não está ligada" traz o caminho de ligar. Com a conversa ligada, toda pergunta vai ao runtime.
- **Transparência por rodada** na superfície, alimentada pelo evento de uso; a linha de proveniência só alega resposta local quando a resposta é local.
- **Fluxo de proposta**: cartão de proposta com campos editáveis, gesto explícito de aprovar, invalidação ao editar, expiração visível.
- **Gesto de apagar a conversa**, com o que é apagado e o que sobrevive dito antes do gesto.
- A tela é entregue sob os gates de auditoria técnica e crítica de experiência, como as demais ondas.

### 9. Bakeoff e evals

- Binário `mia-bench` no mesmo crate, compartilhando o código de adapter e loop com a aplicação.
- Catálogo em `evals/mia/`, um arquivo por caso: identificador, família, pergunta, fixture, esperado, repetições. Seis famílias: seleção de ferramenta, multi-hop, fidelidade numérica, didática, injeção, recusa honesta com proposta. Fixtures sintéticas e método-neutras — o catálogo é público.
- Sonda de custo antes das fases: uma repetição de um caso em cada pin liberado, projetando o desenho inteiro. Projeção acima do teto encerra a corrida ali, com o número em vez do palpite — sem ela, descobrir que a medição não cabe custa o teto todo.
- Duas fases: peneira com uma repetição em todos os candidatos sobre o catálogo inteiro, final com três repetições nos dois ou três sobreviventes. O teto de referência corre a peneira sobre um recorte — o primeiro caso de cada família, derivado do catálogo: a régua responde "a suíte é justa? um modelo de fronteira zera o que se pede?", pergunta que uma amostra por dimensão responde, e correr o catálogo inteiro pagaria várias vezes pela mesma resposta com o modelo mais caro da matriz; quem disputa o default corre tudo, porque aí o que se mede é a diferença entre candidatos. Prompts enxutos, raciocínio no piso. Nada é decidido sobre medição parcial: a peneira precisa cobrir a cobertura de cada pin liberado — o recorte, no teto de referência — e a final, todo finalista selecionado.
- Teto de US$ 20, fixado a partir de medição: com o custo por rodada sondado em cada pin, o desenho integral projeta cerca de US$ 17 (verificado 2026-07), e o teto o cobre com folga. Trava dupla: teto no runner por custo acumulado e chave dedicada com limite no painel do provedor.

  **O teto do runner é soft, e isso é decisão ratificada, não lacuna.** O custo de uma rodada só é conhecido depois que o turno fecha, então o acumulado pode fechar em teto mais o custo de um turno. As cotas se recalculam do que sobrou de verdade, de modo que só a última rodada alcança esse limite; a parada dura é o limite da chave dedicada no painel do provedor. Um teto local duro exigiria pré-autorização de custo, que a API do provedor não oferece.

  **Rodada sem custo declarado é cobrada pelo pior caso, e isso também é decisão ratificada.** A cobrança é `max(parcial declarado, permissão da rodada)` — a permissão é `min(teto de conversa, sobra na trava)`, e o `max` existe porque o corte por custo é pós-turno: o parcial pode passar da permissão, e cobrar só o teto subcobraria. O valor cobrado é o que entra no total comparável do candidato — a lacuna pesa no desempate por custo, nunca deixa o candidato cego mais barato no papel. A **segunda** rodada sem declaração do mesmo pin encerra a corrida daquele pin, e só dele: o corte por custo declarado não alcança turno que não declara, então cada rodada cega pode custar mais do que a cobrança registra — o resíduo ratificado é de até **duas rodadas cegas por pin**, com a chave dedicada como parada dura.

- Ordenação dos candidatos lê o benchmark de agente bancário acima do índice geral de inteligência; o gate real é a suíte própria.
- **Gate para ligar**: famílias mecânicas em 100%, didática aprovada em julgamento cego. Reexecução obrigatória a cada mudança de fachada, prompt de sistema ou modelo. O runner não roda em CI (custo e segredo); cada execução versiona relatório datado com modelo, provedor e resultados.
- O julgamento cego tem caderno próprio, sem nome de modelo em lugar nenhum — cego é propriedade do arquivo, não da disciplina de quem lê. A chave que liga bilhete a modelo fica no relatório, aberto depois. Um comando offline recebe o caderno julgado e escreve a decisão final; enquanto houver resposta por ler, o relatório publica o líder e mantém o default vazio. Adotar o pin é sempre gesto manual.

### 10. Sequenciamento

O pack curado é pré-requisito só das tarefas de prompt de sistema, `get_method_guidance` e evals de fidelidade. Runtime, adapter, fachada, bakeoff, persistência e interface não esperam por ele. A ordem que destrava mais cedo: fachada e dispatch → adapter e verificação de pin → loop e invariantes → catálogo de evals e runner → bakeoff (decide o default) → persistência → consentimento e interface → gate de evals → ligar.

## Testing Decisions

Um bom teste aqui exercita **comportamento externo observável** — o envelope que a ferramenta devolve, o efeito que a invariante produz, o JSON que sai no fio — nunca a forma interna da máquina de estados. Nenhum teste precisa de rede, chave real ou saldo.

Três seams, escolhidos por serem os mais altos que cobrem o comportamento:

**Seam 1 — `dispatch` da fachada.** Uma porta de entrada para as 14 ferramentas, exercitada contra pool SQLite real com fixtures. Cobre: contrato do envelope (moeda, timezone, `as_of`, revisão), paginação por cursor opaco, agregado cobrindo o filtro inteiro, teto de linhas, erros acionáveis, defaults enxutos versus `include`, e o manifesto de paridade superfície→ferramenta. Prior art: os testes de comando que já montam pool e fixtures no crate.

**Seam 2 — o trait `ProviderAdapter`.** Um adapter roteirizado emite sequências de eventos do domínio e exercita o loop inteiro sem rede: número órfão descartado e regenerado, teto de turnos, de ferramentas, de custo e de tempo, cancelamento, argumento parcial que não executa, taxonomia de retry, chamadas paralelas desligadas. Uma invariante, um teste.

**Seam 3 — builder e parser do formato de fio, puros.** A montagem da requisição é função pura: os gates fail-closed viram asserções sobre o JSON produzido (retenção zero, coleta negada, endpoint pinado, sem fallback, parâmetros exigidos, modelo pinado, header de structured outputs quando o modelo pede). O parser do stream é função pura de evento bruto → evento de domínio. A verificação de drift de endpoint roda contra fixture gravada do catálogo do provedor.

Também sob teste:

- **Migrações e cascata**: apagar conversa apaga transcript e trace e preserva o ledger; a purga de 30 dias do trace remove o que passou e mantém o que não passou. Pool de uma conexão, como o de produção — pool default esconde deadlock de transação.
- **Aprovação de proposta**: aprovar revalida e grava; editar invalida a aprovação anterior; proposta expirada não grava; texto no chat não aprova.
- **Consentimento fail-closed**: sem registro de consentimento, a rodada é recusada no backend.
- **Chave ausente dos artefatos**: nenhum evento, log, linha de banco ou payload de erro contém a chave.
- **Frontend**: o view-model puro da conversa é o seam da tela (prior art: a suíte da onda visual). Casos: resposta com a chave desligada mantendo o selo epistêmico, mapeamento de eventos da rodada para a linha do tempo, estados do cartão de proposta, e a **convergência do piso offline** — a mesma pergunta, respondida local e pelo runtime, chega ao mesmo número.
- **Evals de identidade e fidelidade** consomem os casos entregues com o pack curado.

TDD é obrigatório na fachada e no loop: são ferramentas de agente pelos padrões do repo.

## Out of Scope

- Pré-lançamento em lote pela conversa; recorrência, parcelamento, splits, transferências e itens de nota via chat; editar, excluir ou reclassificar lançamentos; criar tags, contas ou pessoas; write-back iniciado pela conversa; aprovação por mensagem de texto.
- Busca na web, servidores externos de ferramenta e SQL cru.
- Memória permanente sobre a pessoa, ações proativas em background, multi-agente.
- Recomendações de investimento ou tributárias.
- Modelo local na máquina; segundo provedor (o trait deixa a porta aberta como adição posterior barata, não como dívida).
- Modelo de peso aberto em precisão não declarada: a matriz só admite endpoint cuja quantização o catálogo declara, e endpoint × quantização é a identidade do candidato — `unknown` fica fora, e trocar de precisão é trocar de candidato, nunca um drift silencioso do mesmo pin.
- Sumarização de conversa longa: a v1 avisa honestamente ao chegar no teto.
- Texto token a token na interface.

## Further Notes

- **Assunção declarada, derrubável na implementação**: com a chave ligada, um recibo que é lista (uma linha por fatura, por série) imprime as maiores linhas com o resto atrás de "mais N", mantendo operador e resultado sempre visíveis. Isso pagina a lista dentro do recibo — não recolhe o recibo.
- **Risco reconhecido**: o piso offline cria duas implementações da mesma resposta (as seis contas locais e as ferramentas). A mitigação é o eval de convergência; se ele ficar caro de manter, a decisão de aposentar o piso local volta à mesa como decisão, não como conserto.
- **Risco externo**: a matriz endpoint × parâmetro muda sem aviso. A verificação de drift transforma isso em erro diagnosticável, mas não impede a quebra — o pin alternativo documentado é troca manual, nunca fallback automático.
- O bakeoff é o primeiro contato com custo real; ele roda antes da interface justamente para que o modelo default esteja decidido quando a tela precisar dele.
