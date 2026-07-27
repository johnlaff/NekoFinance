//! Fachada da conversa: a porta única de leitura sobre o domínio.
//!
//! Uma chamada de ferramenta entra, um envelope comum sai. As ferramentas chamam os MESMOS
//! helpers puros que os comandos Tauri chamam — a fachada não é uma segunda implementação do
//! método, é outra porta para a mesma. Duas implementações da mesma régua divergiriam, e a
//! resposta da conversa deixaria de bater com a da tela.
//!
//! Tudo aqui é somente leitura. Nenhuma ferramenta escreve, e é essa ausência que torna a
//! defesa contra injeção estrutural em vez de censura de conteúdo.

pub(crate) mod catalog;
pub(crate) mod consent;
pub(crate) mod envelope;
pub(crate) mod key_store;
mod ledger_tools;
mod method_tools;
pub(crate) mod provider;
pub(crate) mod run;
mod scenario_tools;
mod state_tools;
mod time_tools;

use catalog::ToolSpec;
use chrono::NaiveDate;
use envelope::{Clock, Envelope, ErrorCode, Meta, Period, ToolError, ToolResult, data_revision};
use method_tools::MethodPack;
use serde_json::Value;
use sqlx::SqlitePool;

/// A chamada como o modelo a emite: nome e argumentos crus, ainda não validados.
pub(crate) struct ToolCall {
    pub name: String,
    pub arguments: Value,
}

impl ToolCall {
    pub(crate) fn new(name: impl Into<String>, arguments: Value) -> Self {
        Self {
            name: name.into(),
            arguments,
        }
    }
}

/// Argumentos já validados contra o catálogo. Construí-los é a validação: uma ferramenta nunca
/// vê argumento que não declarou.
pub(crate) struct Args {
    includes: Vec<String>,
    values: serde_json::Map<String, Value>,
}

impl Args {
    fn parse(spec: &'static ToolSpec, arguments: &Value) -> Result<Self, ToolError> {
        let object = match arguments {
            Value::Null => {
                return Ok(Self {
                    includes: vec![],
                    values: serde_json::Map::new(),
                });
            }
            Value::Object(map) => map,
            _ => {
                return Err(ToolError::new(
                    ErrorCode::InvalidArgument,
                    "Os argumentos precisam ser um objeto.",
                    format!(
                        "Chame {} com um objeto, por exemplo {{\"include\": [\"{}\"]}}.",
                        spec.name,
                        spec.includes.first().map(|i| i.name).unwrap_or("")
                    ),
                ));
            }
        };

        // Fail-closed: argumento não declarado nunca é ignorado em silêncio. Ignorar seria pior
        // que recusar — o modelo acreditaria ter filtrado o que a ferramenta devolveu inteiro.
        for key in object.keys() {
            if key != "include" && !spec.params.contains(&key.as_str()) {
                let mut accepted = vec!["include"];
                accepted.extend(spec.params);
                return Err(ToolError::new(
                    ErrorCode::UnknownArgument,
                    format!("{} não aceita o argumento \"{key}\".", spec.name),
                    format!("Chame de novo usando só: {}.", accepted.join(", ")),
                ));
            }
        }

        let mut includes = Vec::new();
        if let Some(raw) = object.get("include") {
            let list = raw.as_array().ok_or_else(|| {
                ToolError::new(
                    ErrorCode::InvalidArgument,
                    "\"include\" precisa ser uma lista de nomes.",
                    format!(
                        "Use include: [\"{}\"].",
                        spec.includes.first().map(|i| i.name).unwrap_or("")
                    ),
                )
            })?;
            for item in list {
                let name = item.as_str().ok_or_else(|| {
                    ToolError::new(
                        ErrorCode::InvalidArgument,
                        "Cada item de \"include\" precisa ser texto.",
                        format!("Expansões disponíveis: {}.", spec.include_menu()),
                    )
                })?;
                if spec.include(name).is_none() {
                    return Err(ToolError::new(
                        ErrorCode::InvalidArgument,
                        format!("{} não tem a expansão \"{name}\".", spec.name),
                        if spec.includes.is_empty() {
                            format!("{} não aceita expansões — chame sem include.", spec.name)
                        } else {
                            format!("Expansões disponíveis: {}.", spec.include_menu())
                        },
                    ));
                }
                includes.push(name.to_string());
            }
        }
        Ok(Self {
            includes,
            values: object.clone(),
        })
    }

    /// A expansão foi pedida explicitamente?
    pub(crate) fn wants(&self, include: &str) -> bool {
        self.includes.iter().any(|i| i == include)
    }

    /// Valor presente e não-nulo. Um `null` explícito vale como argumento ausente — o modelo
    /// preenche campo opcional com nulo o tempo todo, e recusar isso seria atrito sem ganho.
    fn value(&self, key: &str) -> Option<&Value> {
        self.values.get(key).filter(|v| !v.is_null())
    }

    fn invalid(key: &str, expected: &str, example: impl std::fmt::Display) -> ToolError {
        ToolError::new(
            ErrorCode::InvalidArgument,
            format!("O argumento \"{key}\" precisa ser {expected}."),
            format!("Chame de novo com {example}."),
        )
    }

    /// Mês no formato `YYYY-MM` → (ano, mês).
    pub(crate) fn month(&self, key: &str) -> Result<Option<(i32, u32)>, ToolError> {
        let Some(raw) = self.value(key) else {
            return Ok(None);
        };
        let refuse = || {
            Self::invalid(
                key,
                "um mês no formato YYYY-MM",
                format!("{key}: \"2026-07\""),
            )
        };
        let text = raw.as_str().ok_or_else(refuse)?;
        let (year, month) = text.split_once('-').ok_or_else(refuse)?;
        let year: i32 = year.parse().map_err(|_| refuse())?;
        let month: u32 = month.parse().map_err(|_| refuse())?;
        if !(1..=12).contains(&month) {
            return Err(refuse());
        }
        Ok(Some((year, month)))
    }

    pub(crate) fn year(&self, key: &str) -> Result<Option<i32>, ToolError> {
        let Some(raw) = self.value(key) else {
            return Ok(None);
        };
        let year = raw
            .as_i64()
            .ok_or_else(|| Self::invalid(key, "um ano em número", format!("{key}: 2026")))?;
        Ok(Some(year as i32))
    }

    pub(crate) fn text(&self, key: &str) -> Result<Option<&str>, ToolError> {
        let Some(raw) = self.value(key) else {
            return Ok(None);
        };
        raw.as_str()
            .map(Some)
            .ok_or_else(|| Self::invalid(key, "texto", format!("{key}: \"…\"")))
    }

    /// Valor em centavos. Dinheiro só entra inteiro: um float aqui viraria centavo perdido no
    /// arredondamento, e o recibo não pega o que a comparação já comeu.
    pub(crate) fn cents(&self, key: &str) -> Result<Option<i64>, ToolError> {
        let Some(raw) = self.value(key) else {
            return Ok(None);
        };
        raw.as_i64().map(Some).ok_or_else(|| {
            Self::invalid(
                key,
                "um valor em centavos inteiros",
                format!("{key}: 50000 (R$ 500,00)"),
            )
        })
    }

    /// Lista de objetos: cada mudança preserva seus campos para a ferramenta validar a própria
    /// gramática, sem aceitar valores soltos que não têm como ser interpretados.
    pub(crate) fn objects(
        &self,
        key: &str,
    ) -> Result<Vec<&serde_json::Map<String, Value>>, ToolError> {
        let Some(raw) = self.value(key) else {
            return Ok(vec![]);
        };
        let list = raw.as_array().ok_or_else(|| {
            Self::invalid(
                key,
                "uma lista de objetos",
                format!("{key}: [{{\"campo\": \"valor\"}}]"),
            )
        })?;
        list.iter()
            .enumerate()
            .map(|(index, item)| {
                item.as_object().ok_or_else(|| {
                    ToolError::new(
                        ErrorCode::InvalidArgument,
                        format!("O item #{} de \"{key}\" precisa ser um objeto.", index + 1),
                        format!("Chame de novo com {key}: [{{\"campo\": \"valor\"}}]."),
                    )
                })
            })
            .collect()
    }

    /// Palavra de um vocabulário fechado. A recusa lista o vocabulário inteiro: corrigir uma
    /// palavra errada nunca depende de adivinhar a palavra certa.
    pub(crate) fn choice(
        &self,
        key: &str,
        vocabulary: &[&'static str],
    ) -> Result<Option<&'static str>, ToolError> {
        let Some(raw) = self.text(key)? else {
            return Ok(None);
        };
        vocabulary
            .iter()
            .find(|word| **word == raw)
            .copied()
            .map(Some)
            .ok_or_else(|| {
                ToolError::new(
                    ErrorCode::InvalidArgument,
                    format!("\"{raw}\" não é um valor de \"{key}\"."),
                    format!("Chame de novo com {key} em: {}.", vocabulary.join(", ")),
                )
            })
    }

    /// Recorte de datas explícitas (`{"start": "…", "end": "…"}`). O vocabulário da fachada não
    /// tem "últimos 30 dias": quem chama diz as duas pontas, e a resposta responde por elas.
    pub(crate) fn range(&self, key: &str) -> Result<Option<(NaiveDate, NaiveDate)>, ToolError> {
        let Some(raw) = self.value(key) else {
            return Ok(None);
        };
        let refuse = || {
            Self::invalid(
                key,
                "um recorte com as duas datas em YYYY-MM-DD",
                format!("{key}: {{\"start\": \"2026-07-01\", \"end\": \"2026-12-31\"}}"),
            )
        };
        let object = raw.as_object().ok_or_else(refuse)?;
        let date = |field: &str| -> Result<NaiveDate, ToolError> {
            let text = object
                .get(field)
                .and_then(|v| v.as_str())
                .ok_or_else(refuse)?;
            NaiveDate::parse_from_str(text, "%Y-%m-%d").map_err(|_| refuse())
        };
        let (start, end) = (date("start")?, date("end")?);
        if end < start {
            return Err(ToolError::new(
                ErrorCode::InvalidArgument,
                format!("O recorte \"{key}\" termina antes de começar."),
                "Chame de novo com start anterior ou igual a end.".to_string(),
            ));
        }
        Ok(Some((start, end)))
    }
}

/// O mundo de uma rodada: o relógio que carimba as respostas e onde o pack curado do método
/// está montado. Uma leitura só do relógio por rodada, um caminho só para o conteúdo servido.
pub(crate) struct Context {
    pub clock: Clock,
    pub pack: MethodPack,
}

/// Acrescenta um campo ao objeto de dados de uma ferramenta. A serialização de um tipo próprio
/// não falha; um `json!` mal formado seria erro de programação, não de dado.
pub(crate) fn insert(data: &mut Value, key: &str, value: impl serde::Serialize) {
    let object = data
        .as_object_mut()
        .expect("dados de ferramenta são sempre um objeto");
    object.insert(
        key.to_string(),
        serde_json::to_value(value).expect("dados de ferramenta são serializáveis"),
    );
}

/// A porta. Despacha a chamada e devolve o envelope — inclusive quando falha: erro de
/// ferramenta é resposta, não exceção, porque o modelo precisa lê-lo para se corrigir na mesma
/// rodada.
pub(crate) async fn dispatch(pool: &SqlitePool, call: &ToolCall, ctx: &Context) -> Envelope {
    let revision = data_revision(pool).await.ok();
    let today = ctx.clock.today();

    let Some(spec) = catalog::spec(&call.name) else {
        return envelope_for(
            &call.name,
            ctx.clock,
            revision,
            Period::day(today),
            Err(ToolError::new(
                ErrorCode::UnknownTool,
                format!("Não existe a ferramenta \"{}\".", call.name),
                format!("Escolha uma destas: {}.", catalog::tool_names().join(", ")),
            )),
        );
    };

    let outcome = match Args::parse(spec, &call.arguments) {
        Err(e) => Err(e),
        Ok(args) => run(pool, spec, &args, today, ctx).await,
    };

    let period = match &outcome {
        Ok(out) => out.period.clone(),
        Err(_) => Period::day(today),
    };
    envelope_for(spec.name, ctx.clock, revision, period, outcome)
}

/// Envelope de recusa montado pelo laço ANTES de a ferramenta rodar. A validação local que falha
/// nunca executa, e o modelo recebe a recusa no mesmo formato de qualquer outra resposta — é isso
/// que lhe permite se corrigir na mesma rodada.
pub(crate) async fn refuse(
    pool: &SqlitePool,
    tool: &str,
    ctx: &Context,
    error: ToolError,
) -> Envelope {
    envelope_for(
        tool,
        ctx.clock,
        data_revision(pool).await.ok(),
        Period::day(ctx.clock.today()),
        Err(error),
    )
}

async fn run(
    pool: &SqlitePool,
    spec: &'static ToolSpec,
    args: &Args,
    today: chrono::NaiveDate,
    ctx: &Context,
) -> ToolResult {
    match spec.name {
        "get_financial_snapshot" => state_tools::financial_snapshot(pool, args, today).await,
        "get_data_status" => state_tools::data_status(pool, args, today).await,
        "get_budget_settings" => state_tools::budget_settings(pool, args, today).await,
        "get_accounts_and_net_worth" => {
            state_tools::accounts_and_net_worth(pool, args, today).await
        }
        "get_month_analysis" => time_tools::month_analysis(pool, args, today).await,
        "get_year_analysis" => time_tools::year_analysis(pool, args, today).await,
        "get_forecast" => time_tools::forecast(pool, args, today).await,
        "get_cashflow_calendar" => time_tools::cashflow_calendar(pool, args, today).await,
        "search_transactions" => ledger_tools::search_transactions(pool, args, today).await,
        "get_tags" => ledger_tools::tags(pool, args, today).await,
        "get_commitments" => ledger_tools::commitments(pool, args, today).await,
        "simulate_scenario" => scenario_tools::simulate_scenario(pool, args, today).await,
        "get_method_guidance" => method_tools::method_guidance(&ctx.pack, args, today).await,
        // O catálogo é a fonte da verdade dos nomes; uma entrada sem braço aqui é erro de
        // programação, e o teste de cobertura do catálogo o pega antes de qualquer rodada.
        other => Err(ToolError::new(
            ErrorCode::UnknownTool,
            format!("A ferramenta \"{other}\" está declarada mas não foi ligada."),
            "Use outra ferramenta do catálogo.".to_string(),
        )),
    }
}

fn envelope_for(
    tool: &str,
    clock: Clock,
    data_revision: Option<String>,
    period: Period,
    outcome: ToolResult,
) -> Envelope {
    let meta = Meta {
        currency: envelope::CURRENCY,
        timezone: clock.timezone(),
        period,
        as_of: clock.as_of(),
        data_revision,
        row_limit: envelope::MAX_ROWS,
    };
    match outcome {
        Ok(out) => Envelope {
            tool: tool.to_string(),
            ok: true,
            meta,
            data: Some(out.data),
            error: None,
        },
        Err(error) => Envelope {
            tool: tool.to_string(),
            ok: false,
            meta,
            data: None,
            error: Some(error),
        },
    }
}

#[cfg(test)]
mod tests;
