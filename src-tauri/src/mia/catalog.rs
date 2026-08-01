//! Catálogo das ferramentas da conversa.
//!
//! Cada entrada declara "use para" e "não use para". A segunda metade é a que evita a chamada
//! errada: sem ela o modelo pede o teto do diário à ferramenta de saldo e responde com o número
//! de outra régua. O catálogo também é a lista fechada de argumentos aceitos — validação
//! fail-closed lê daqui, não de cada ferramenta.

/// Expansão opcional de uma ferramenta. Default enxuto é regra: campo pesado (lista longa,
/// projeção do horizonte) só sai quando pedido pelo nome.
pub(crate) struct Include {
    pub name: &'static str,
    pub description: &'static str,
}

pub(crate) struct ToolSpec {
    pub name: &'static str,
    pub summary: &'static str,
    pub use_for: &'static str,
    pub not_for: &'static str,
    /// Argumentos próprios da ferramenta, além de `include` (aceito por todas).
    pub params: &'static [&'static str],
    pub includes: &'static [Include],
}

impl ToolSpec {
    pub(crate) fn include(&self, name: &str) -> Option<&'static Include> {
        self.includes.iter().find(|i| i.name == name)
    }

    pub(crate) fn include_names(&self) -> Vec<&'static str> {
        self.includes.iter().map(|i| i.name).collect()
    }

    /// Expansões com o que cada uma traz — é o que o erro devolve, para que a correção não
    /// dependa de o modelo adivinhar qual expansão serve.
    pub(crate) fn include_menu(&self) -> String {
        self.includes
            .iter()
            .map(|i| format!("{} ({})", i.name, i.description))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

pub(crate) const CATALOG: &[ToolSpec] = &[
    ToolSpec {
        name: "get_financial_snapshot",
        summary: "Retrato de agora: saldo projetado do mês, teto e gasto do dia, reserva, modo \
                  de gasto e próxima fatura.",
        use_for: "Perguntas sobre como a pessoa está AGORA — \"como estou\", \"quanto gastei \
                  hoje\", \"quanto tenho de reserva\", \"qual o saldo do fim do mês\", \"quando \
                  vence a próxima fatura\".",
        not_for: "Fechamento de um mês passado ou comparação entre meses (use get_month_analysis); \
                  régua anual do Economizado% (use get_year_analysis); lista de lançamentos (use \
                  search_transactions).",
        params: &[],
        includes: &[
            Include {
                name: "upcoming_invoices",
                description: "Próxima fatura de cada cartão, com vencimento, valor e status.",
            },
            Include {
                name: "guardrail",
                description: "Quanto ainda dá para gastar hoje e qual régua está limitando \
                              (caixa ou poupança). Roda a projeção do horizonte.",
            },
        ],
    },
    ToolSpec {
        name: "get_data_status",
        summary: "O que existe e o que falta de dado: cobertura, última importação, estados \
                  epistêmicos das réguas e pendências que esperam um gesto da pessoa.",
        use_for: "Antes de recusar por falta de dado, e para responder \"por que esse número não \
                  aparece\", \"o app está atualizado?\", \"o que preciso preencher?\".",
        not_for: "Ler os números em si — esta ferramenta diz o ESTADO do dado, nunca o valor das \
                  réguas.",
        params: &[],
        includes: &[Include {
            name: "future_coverage",
            description: "Quanto de cada mês futuro já foi pré-lançado ante o gasto típico. \
                          Roda a projeção do horizonte.",
        }],
    },
    ToolSpec {
        name: "get_budget_settings",
        summary: "O teto do Diário vigente com sua procedência, a cerimônia que o produziu e as \
                  metas do método (faixa de economia, meses de reserva).",
        use_for: "Perguntas sobre o LIMITE combinado — \"qual meu teto\", \"de onde veio esse \
                  valor\", \"quanto o método pede de economia\", \"tem proposta de teto me \
                  esperando?\".",
        not_for: "Quanto foi gasto hoje contra o teto (use get_financial_snapshot); gasto por \
                  categoria, que o método não orça (use get_tags para entender as réguas).",
        params: &[],
        includes: &[Include {
            name: "ceremony",
            description: "Itens mensais do teto e a nota da planilha que documenta a cerimônia.",
        }],
    },
    ToolSpec {
        name: "get_accounts_and_net_worth",
        summary: "Onde o dinheiro está: totais por classe de liquidez e patrimônio somado.",
        use_for: "Perguntas sobre ONDE o dinheiro está — \"quanto tenho no banco\", \"quanto está \
                  na reserva\", \"qual meu patrimônio\", \"quais contas eu tenho\".",
        not_for: "Faturas, limite ou dívida de cartão, que não são bolso (use get_commitments); \
                  saldo projetado do fim do mês (use get_financial_snapshot).",
        params: &[],
        includes: &[Include {
            name: "accounts",
            description: "Uma linha por conta, com tipo, liquidez, instituição e saldo.",
        }],
    },
    ToolSpec {
        name: "get_month_analysis",
        summary: "Um mês fechado nas contas do método: entradas, os cinco baldes de saída, custo \
                  de vida, performance e Economizado% — com um mês de comparação opcional.",
        use_for: "Perguntas sobre UM mês — \"como foi maio\", \"quanto gastei no mês passado\", \
                  \"maio contra abril\", \"quanto sobrou em junho\", \"como está o mês corrente\".",
        not_for: "Julgar se a economia do mês está boa: a faixa 20–30% é ANUAL (use \
                  get_year_analysis); saldo dia a dia (use get_cashflow_calendar); lista de \
                  lançamentos do mês (use search_transactions).",
        params: &["month", "compare_to"],
        includes: &[
            Include {
                name: "days",
                description: "Um dia por linha, como a planilha os escreve: entrada, saída fixa, \
                              diário e o Saldo encadeado.",
            },
            Include {
                name: "owners",
                description: "Quanto cada titular respondeu no mês (divisão de despesas).",
            },
        ],
    },
    ToolSpec {
        name: "get_year_analysis",
        summary: "O ano na régua do método: Economizado% contra a faixa 20–30%, o recorte que \
                  sustenta o número, os meses sem lastro e um ano de comparação opcional.",
        use_for: "Perguntas sobre o ANO e sobre o veredito da economia — \"estou dentro da \
                  faixa?\", \"quanto guardei este ano\", \"2026 contra 2025\", \"quanto falta \
                  para os 20%\".",
        not_for: "Fechamento de um mês (use get_month_analysis); saldo projetado à frente (use \
                  get_forecast); comparar a renda TOTAL de um ano em curso com a de um ano \
                  fechado, que acusaria uma queda inexistente — a comparação honesta entre anos é \
                  a renda média por mês com registro, e ela já vem pronta.",
        params: &["year", "compare_to"],
        includes: &[
            Include {
                name: "months",
                description: "Os doze meses do ano, com entrada, saída, economia, performance e \
                              a marca de vivido/sem lastro.",
            },
            Include {
                name: "year_end",
                description: "Onde o ano termina: saldo do último mês projetado e o cenário em \
                              que os meses sem lastro custassem o típico. Roda a projeção.",
            },
        ],
    },
    ToolSpec {
        name: "get_forecast",
        summary: "A projeção do caixa daqui para frente: saldo por mês, menor saldo do recorte, \
                  quanto dá para gastar hoje e o que ainda falta lançar.",
        use_for: "Perguntas sobre o FUTURO — \"tem buraco à frente?\", \"como termino o ano\", \
                  \"quanto posso gastar hoje\", \"e se eu financiar?\" (com scenario_id).",
        not_for: "Meses já vividos (use get_month_analysis ou get_cashflow_calendar); o dia a dia \
                  da projeção quando a pergunta é sobre uma data (use get_cashflow_calendar).",
        params: &["range", "scenario_id"],
        includes: &[
            Include {
                name: "daily",
                description: "Saldo projetado dia a dia dentro do recorte.",
            },
            Include {
                name: "coverage",
                description: "Por mês futuro, quanto do gasto típico já foi lançado — o que diz \
                              se a projeção daquele mês é crível.",
            },
        ],
    },
    ToolSpec {
        name: "search_transactions",
        summary: "Os lançamentos de um recorte, filtrados por período, valor, conta, tag, pessoa \
                  responsável, forma de pagamento e natureza — com os totais do filtro inteiro e \
                  paginação por cursor.",
        use_for: "Perguntas de recorte próprio, sobre lançamentos — \"quanto gastei com esta \
                  conta em maio\", \"o que passou de R$ 500\", \"o que a outra pessoa respondeu\", \
                  \"o que foi no crédito\", \"quais são as fixas do mês\".",
        not_for: "Os baldes do mês nas contas do método (use get_month_analysis): a soma das \
                  linhas filtradas não é o custo de vida, que o motor compõe com as faturas e as \
                  máscaras de tag. Saldo por dia (use get_cashflow_calendar). O que já está \
                  comprometido à frente (use get_commitments).",
        params: &[
            "range",
            "min_cents",
            "max_cents",
            "account_id",
            "tag_id",
            "owner_person_id",
            "payment_method",
            "nature",
            "sort",
            "cursor",
        ],
        includes: &[
            Include {
                name: "tags",
                description: "As tags de cada linha.",
            },
            Include {
                name: "items",
                description: "As partes itemizadas da nota de cada linha, com o balde de cada uma.",
            },
            Include {
                name: "owners",
                description: "Quem respondeu por cada linha (divisão de despesas).",
            },
        ],
    },
    ToolSpec {
        name: "get_tags",
        summary: "As tags de um mês: em quais réguas cada uma conta, o que movimentou, o preço \
                  das exceções no custo de vida e o dinheiro de terceiros.",
        use_for: "Perguntas sobre TAGS e sobre dinheiro que não é seu — \"quais tags puxaram o \
                  mês\", \"o que está fora do custo de vida\", \"quanto saiu em nome de outra \
                  pessoa\", \"quanto ainda me devem\".",
        not_for: "Orçar por categoria: o método não orça por categoria, e a tag não é envelope de \
                  gasto — ela é interruptor de contabilidade, e decide em quais réguas o \
                  lançamento conta. O custo de vida do mês nas contas do método vem de \
                  get_month_analysis; os lançamentos de uma tag, de search_transactions.",
        params: &["month"],
        includes: &[
            Include {
                name: "effects",
                description: "Quanto cada régua mexeria se o interruptor daquela tag fosse ligado.",
            },
            Include {
                name: "third_parties",
                description: "Uma linha por pessoa: o que saiu em nome dela, o que voltou, o que \
                              ainda se espera e em que estado está.",
            },
        ],
    },
    ToolSpec {
        name: "get_commitments",
        summary: "O que já está comprometido nos ciclos à frente: parcelamentos e assinaturas do \
                  cartão, séries do Livro-razão e obrigações nomeadas — com a parcela n/N e o \
                  reembolso vinculado.",
        use_for: "Perguntas sobre o que JÁ ESTÁ PROMETIDO — \"quais parcelas ainda tenho\", \
                  \"quanto sai por mês em assinatura\", \"quanto falta do notebook\", \"o \
                  aluguel subiu?\" (com obligation_id).",
        not_for: "O saldo que sobra depois disso (use get_forecast); o total de uma fatura e o \
                  próximo vencimento (use get_financial_snapshot); os lançamentos já feitos (use \
                  search_transactions).",
        params: &["range", "obligation_id"],
        includes: &[Include {
            name: "occurrences",
            description: "Uma linha por parcela ou ocorrência dentro do recorte, com data, \
                          valor e posição na série.",
        }],
    },
    ToolSpec {
        name: "get_cashflow_calendar",
        summary: "O caixa dia a dia num recorte de datas: movimento do dia, saldo que ele deixou \
                  e o menor saldo do período. Passado lê a planilha, futuro lê a projeção.",
        use_for: "Perguntas ancoradas em DATA — \"como fica o saldo até o dia 10\", \"qual o pior \
                  dia do mês\", \"o que acontece na semana que vem\", \"o saldo aguenta até o \
                  salário?\".",
        not_for: "Totais do mês nas contas do método (use get_month_analysis); os lançamentos de \
                  um dia (use search_transactions).",
        params: &["range", "cursor"],
        includes: &[],
    },
    ToolSpec {
        name: "simulate_scenario",
        summary: "Uma hipótese jogada por cima do mundo real, com os dois lados projetados pelo \
                  mesmo motor e as diferenças já feitas. NADA é gravado: a simulação não cria \
                  cenário nem lançamento, e não há o que apagar depois. Cada mudança é \
                  {movement, amount_cents, date} — movement em entrada · saida · diario · \
                  cartao · economia · patrimonio, valor sempre positivo (o sinal vem do \
                  movement) — mais os opcionais repeat_months (repete a linha mês a mês; as \
                  datas são contadas pela ferramenta) e description.",
        use_for: "Perguntas de \"e se\" — \"dá para assumir uma parcela de R$ 800?\", \"e se eu \
                  cortar R$ 300 por mês?\", \"e se eu guardar mais R$ 500 todo mês?\", \"esse \
                  gasto abre buraco lá na frente?\".",
        not_for: "A projeção do mundo como ele está, sem hipótese nenhuma (use get_forecast); um \
                  cenário SALVO, que tem nome e vive na tela Horizonte (use get_forecast com \
                  scenario_id). Simular também não registra: um lançamento que a pessoa QUER \
                  passa pelo gesto de aprovar, nunca por uma simulação.",
        params: &["changes"],
        includes: &[Include {
            name: "month_end",
            description: "Saldo de fim de mês nos dois mundos, com a diferença.",
        }],
    },
    ToolSpec {
        name: "get_method_guidance",
        summary: "O capítulo do método sobre um tópico, servido inteiro do material curado \
                  local. Tópicos: metodo · diario · cartao · economia · reserva · dividas · \
                  financiamento · patrimonio · renda · casal · planejamento.",
        use_for: "Perguntas sobre o MÉTODO, não sobre os números — \"o que é o Diário?\", \"por \
                  que a reserva vem antes de investir?\", \"como o método trata dívida?\", \"me \
                  explica a faixa de economia\", \"o que o método diz sobre financiamento?\".",
        not_for: "Qualquer número da pessoa, que vem das outras ferramentas: esta camada \
                  EXPLICA e nunca calcula. A resposta que nasce daqui é explicação do método, e \
                  precisa se apresentar como tal — jamais como conta feita sobre os lançamentos \
                  de quem perguntou.",
        params: &["topic"],
        includes: &[],
    },
    ToolSpec {
        name: "propose_transaction",
        summary: "Monta a proposta de UM lançamento avulso — entrada ou despesa — já validada e \
                  normalizada, com validade e assinatura. Nada é gravado: a proposta vira um \
                  cartão na tela, e só o gesto de aprovar cria o lançamento. Campos: kind \
                  (income · expense), amount_cents (inteiro positivo), date (YYYY-MM-DD), mais os \
                  opcionais description, payment_method, is_fixed e tag_ids.",
        use_for: "Quando a pessoa PEDE para registrar um gasto ou uma entrada avulsa — \"lança \
                  aí R$ 80 de farmácia hoje\", \"registra o salário de sexta\", \"anota 45 reais \
                  de uber ontem\".",
        not_for: "Transferência, Economia, recorrência, parcelamento e divisão entre pessoas, que \
                  o formulário de Lançar trata direito e esta ferramenta recusa. Editar, excluir \
                  ou reclassificar lançamento que já existe. E aprovar: texto no chat não aprova \
                  nada — quem cria o lançamento é o gesto da pessoa sobre o cartão.",
        params: &[
            "kind",
            "amount_cents",
            "date",
            "description",
            "payment_method",
            "is_fixed",
            "tag_ids",
        ],
        includes: &[],
    },
];

pub(crate) fn spec(name: &str) -> Option<&'static ToolSpec> {
    CATALOG.iter().find(|t| t.name == name)
}

/// A única ferramenta da camada de método: ela explica e nunca calcula. É o que separa uma
/// resposta didática de uma conta sobre os números de quem perguntou.
pub(crate) const METHOD_LAYER_TOOL: &str = "get_method_guidance";

/// A única ferramenta que monta um lançamento canônico em vez de responder uma leitura. Ela
/// continua read-only: valida, normaliza e devolve a proposta assinada — quem grava é o gesto de
/// aprovação, fora do laço.
pub(crate) const PROPOSAL_TOOL: &str = "propose_transaction";

pub(crate) fn is_method_layer(tool: &str) -> bool {
    tool == METHOD_LAYER_TOOL
}

pub(crate) fn tool_names() -> Vec<&'static str> {
    CATALOG.iter().map(|t| t.name).collect()
}
