//! Suíte do prefixo estável: o que está sob teste é o TEXTO que sai da máquina e o orçamento que
//! ele ocupa, nunca a ordem interna da montagem.

use super::*;
use crate::mia::test_pack::TempPack;

/// Um pack instalado: núcleo curado mais a deny-list que o gate exige para servi-lo.
fn installed_pack() -> TempPack {
    let pack = TempPack::new();
    pack.core("# Núcleo do método\n\nA faixa anual de economia é de 20–30% das entradas.\n");
    pack.root_file("forbidden-extra.txt", "termo-ausente-da-fixture\n");
    pack
}

/// O dia que a suíte injeta — o mesmo do relógio da bancada, para que os textos batam.
fn today() -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(2026, 7, 25).expect("data fixa da suíte")
}

/// O prefixo carrega o dia do relógio INJETADO, nunca do relógio ambiente: sem a âncora, o modelo
/// não tem como resolver "julho" antes da primeira ferramenta — ou pergunta o óbvio, ou chuta o
/// ano do treino.
#[tokio::test]
async fn the_prefix_anchors_today_from_the_injected_clock() {
    let pack = installed_pack();

    let prompt = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .unwrap();
    assert!(
        prompt.text.contains("Hoje é 2026-07-25."),
        "prefixo: {}",
        prompt.text
    );

    let other_day = chrono::NaiveDate::from_ymd_opt(2031, 1, 2).unwrap();
    let other = system_prompt(&MethodPack::at(pack.path()), other_day)
        .await
        .unwrap();
    assert!(other.text.contains("Hoje é 2031-01-02."));
    assert!(!other.text.contains("2026-07-25"));
}

/// A âncora resolve a data incompleta; a ambiguidade REAL continua virando pergunta. As duas
/// regras convivem no mesmo texto — perder qualquer uma reabre o defeito da outra ponta.
#[tokio::test]
async fn date_resolution_keeps_real_ambiguity_asking() {
    let pack = installed_pack();

    let prompt = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .unwrap();

    for expected in [
        "mês sem ano é o do ano corrente",
        "que o calendário de hoje não resolve",
        "Pergunte. Nunca suponha.",
    ] {
        assert!(
            prompt.text.contains(expected),
            "o prefixo perdeu \"{expected}\": {}",
            prompt.text
        );
    }
}

#[tokio::test]
async fn the_prefix_carries_the_method_core_from_the_local_pack() {
    let pack = installed_pack();

    let prompt = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .unwrap();

    assert!(prompt.method_core);
    assert!(
        prompt
            .text
            .contains("A faixa anual de economia é de 20–30%"),
        "prefixo: {}",
        prompt.text
    );
}

#[tokio::test]
async fn the_prefix_adds_the_app_context_and_the_sheet_structure_to_the_core() {
    let pack = installed_pack();

    let prompt = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .unwrap();

    for expected in [
        "# A conversa",
        "# Núcleo do método",
        "# O app e a estrutura dos dados",
        "Tag é interruptor de régua",
        "A célula é canônica para o valor",
        "Dinheiro em centavos inteiros",
    ] {
        assert!(
            prompt.text.contains(expected),
            "o prefixo perdeu \"{expected}\": {}",
            prompt.text
        );
    }
}

#[tokio::test]
async fn the_prefix_is_identical_between_rounds() {
    let pack = installed_pack();

    let first = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .unwrap();
    let second = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .unwrap();

    // Estabilidade é o que dá direito ao desconto de cache: um prefixo que muda a cada rodada é
    // pago inteiro a cada rodada.
    assert_eq!(first, second);
}

#[tokio::test]
async fn a_full_sized_method_core_still_fits_the_prefix_budget() {
    let pack = TempPack::new();
    // O núcleo curado real ocupa dezenas de milhares de caracteres; o teto precisa caber nele com
    // folga, senão a conversa quebra na máquina de quem tem o pack completo.
    pack.core(&"Regra canônica do método, escrita por extenso. ".repeat(700));
    pack.root_file("forbidden-extra.txt", "termo-ausente-da-fixture\n");

    let prompt = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .unwrap();

    assert!(
        prompt.estimated_tokens <= MAX_PREFIX_TOKENS,
        "tokens estimados: {}",
        prompt.estimated_tokens
    );
}

#[tokio::test]
async fn a_method_core_beyond_the_budget_is_refused_instead_of_truncated() {
    let pack = TempPack::new();
    pack.core(&"Regra canônica do método. ".repeat(20_000));
    pack.root_file("forbidden-extra.txt", "termo-ausente-da-fixture\n");

    let error = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .unwrap_err();

    assert!(
        error.message.contains(&MAX_PREFIX_TOKENS.to_string()),
        "mensagem: {}",
        error.message
    );
    assert!(error.fix.contains("Enxugue"), "conserto: {}", error.fix);
}

#[tokio::test]
async fn the_privacy_scan_covers_the_assembled_prefix_and_not_only_the_core() {
    let pack = installed_pack();
    // O termo vive no bloco versionado, não no núcleo: o gate precisa varrer o que SAI da máquina,
    // que é o prefixo inteiro.
    pack.root_file("forbidden-extra.txt", "interruptor de régua\n");

    let error = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .unwrap_err();

    assert!(
        error.message.contains("forbidden-extra.txt") && error.message.contains("#1"),
        "mensagem: {}",
        error.message
    );
    assert!(
        !error.message.contains("interruptor de régua"),
        "o erro repetiu o termo bloqueado: {}",
        error.message
    );
}

#[tokio::test]
async fn a_pack_without_a_deny_list_does_not_serve_the_method_core() {
    let pack = TempPack::new();
    pack.core("# Núcleo do método\n\nRegra canônica.\n");

    let error = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .unwrap_err();

    assert!(
        error.message.contains("deny-list"),
        "mensagem: {}",
        error.message
    );
}

#[tokio::test]
async fn an_empty_deny_list_does_not_serve_the_method_core() {
    let pack = TempPack::new();
    pack.core("# Núcleo do método\n\nRegra canônica.\n");
    pack.root_file("forbidden-extra.txt", "");

    let error = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .expect_err("deny-list sem padrão deve recusar o prefixo");

    assert!(
        error.message.contains("padrão"),
        "mensagem: {}",
        error.message
    );
    assert!(error.fix.contains("deny-list"), "conserto: {}", error.fix);
}

#[tokio::test]
async fn a_comment_only_deny_list_does_not_serve_the_method_core() {
    let pack = TempPack::new();
    pack.core("# Núcleo do método\n\nRegra canônica.\n");
    pack.root_file(
        "forbidden-extra.txt",
        "# Termos privados desta instalação\n\n# Cada linha ativa bloqueia uma expressão\n",
    );

    let error = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .expect_err("deny-list sem padrão deve recusar o prefixo");

    assert!(
        error.message.contains("padrão"),
        "mensagem: {}",
        error.message
    );
    assert!(error.fix.contains("deny-list"), "conserto: {}", error.fix);
}

#[tokio::test]
async fn a_pack_that_is_not_installed_degrades_the_prefix_instead_of_blocking_the_conversation() {
    let pack = TempPack::absent();

    let prompt = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .unwrap();

    assert!(!prompt.method_core);
    assert!(prompt.text.contains("não está montado nesta máquina"));
    assert!(prompt.text.contains("# A conversa"));
    assert!(prompt.text.contains("# O app e a estrutura dos dados"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_broken_core_symlink_refuses_the_prefix_without_exposing_the_pack_path() {
    let pack = TempPack::new();
    let absolute_path = pack.path().display().to_string();
    std::os::unix::fs::symlink("core-ausente.md", pack.path().join("core.md"))
        .expect("o link simbólico quebrado deve existir no pack");

    let error = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .expect_err("núcleo corrompido não pode degradar o prefixo");

    assert!(error.message.contains("núcleo do método"));
    assert!(
        !error.message.contains(&absolute_path),
        "mensagem: {}",
        error.message
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_core_that_links_outside_the_pack_never_reaches_the_prefix() {
    let pack = TempPack::new();
    let outside = TempPack::new();
    outside.root_file("segredo.txt", "conteudo-de-fora-do-pack");
    pack.root_file("forbidden-extra.txt", "termo-ausente-da-fixture\n");
    std::os::unix::fs::symlink(
        outside.path().join("segredo.txt"),
        pack.path().join("core.md"),
    )
    .expect("o link para fora do pack deve existir");

    let error = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .expect_err("o núcleo é um arquivo do próprio pack, nunca um link para fora dele");

    // Um link no lugar do núcleo daria ao pack o poder de despejar no prefixo qualquer arquivo
    // legível pelo app — e o prefixo é justamente o que sai da máquina.
    assert!(error.message.contains("núcleo do método"));
    assert!(
        !error.message.contains("conteudo-de-fora-do-pack"),
        "mensagem: {}",
        error.message
    );
}

#[tokio::test]
async fn the_prefix_forbids_attributing_an_origin_to_the_method() {
    let pack = TempPack::absent();

    let prompt = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .unwrap();

    // A regra de identidade vive no bloco versionado justamente para sobreviver a uma máquina sem
    // o pack: sem ela, a pergunta-armadilha sobre a origem não teria contrato nenhum.
    let identity = prompt.text.replace('\n', " ");
    for expected in [
        "não tem autor, marca, curso, comunidade nem planilha à venda",
        "não atribua, não confirme e não negue",
        "não cita aula, transcrição, material de apoio nem fonte",
    ] {
        assert!(
            identity.contains(expected),
            "o prefixo perdeu \"{expected}\"",
        );
    }
}

#[tokio::test]
async fn the_prefix_frames_the_method_layer_as_explanation_never_as_calculation() {
    let pack = installed_pack();

    let prompt = system_prompt(&MethodPack::at(pack.path()), today())
        .await
        .unwrap();

    assert!(prompt.text.contains("Explicar não é calcular"));
    assert!(
        prompt
            .text
            .contains("jamais como conta sobre os números de quem perguntou")
    );
}

#[test]
fn the_token_estimate_is_pessimistic_at_both_ends() {
    // Superestimar cabe; subestimar só aparece como rodada derrubada pelo provedor.
    assert_eq!(estimate_tokens(""), 0);
    // Texto latino: manda a conta por caracteres.
    assert_eq!(estimate_tokens("ação"), 2);
    // Escrita densa em multibyte: manda a conta por bytes, que aqui é a maior das duas.
    assert_eq!(estimate_tokens("日本語テキスト"), 6);
}
