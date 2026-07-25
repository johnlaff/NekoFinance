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
];

pub(crate) fn spec(name: &str) -> Option<&'static ToolSpec> {
    CATALOG.iter().find(|t| t.name == name)
}

pub(crate) fn tool_names() -> Vec<&'static str> {
    CATALOG.iter().map(|t| t.name).collect()
}
