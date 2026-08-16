//! Trava de imutabilidade das migrações já lançadas. O sqlx grava o checksum SHA-384 de cada
//! migração no banco na primeira vez que ela roda, e recusa ABRIR bancos existentes se o
//! conteúdo do arquivo mudar depois — mesmo que a mudança seja só um comentário (foi exatamente
//! isso que quebrou o app em campo: editar o texto de uma migração já lançada). Este teste pina
//! o checksum de cada migração listada em `migrations/CHECKSUMS.lock` e falha se o arquivo
//! divergir, para pegar o erro no `cargo test` em vez de no aparelho do dono.
//!
//! `CHECKSUMS.lock` CRESCE a cada release: ao lançar uma migração nova, adicione a linha dela
//! (`sha384sum src-tauri/migrations/<arquivo>.sql`) depois que a migração já foi publicada — uma
//! migração ainda não lançada não precisa entrar aqui, e não entrar não quebra este teste (só as
//! linhas listadas são checadas). Corrigir uma migração já lançada (typo, comentário desatualizado)
//! não é uma opção: crie uma migração nova.

use sqlx::migrate::Migration;

/// Uma entrada de `CHECKSUMS.lock`: nome do arquivo (documental) + o checksum SHA-384 (hex) que a
/// migração PRECISA continuar tendo.
struct LockedMigration {
    filename: &'static str,
    version: i64,
    expected_checksum_hex: &'static str,
}

fn parse_lock_file() -> Vec<LockedMigration> {
    const LOCK: &str = include_str!("../migrations/CHECKSUMS.lock");

    LOCK.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut parts = line.split_whitespace();
            let filename = parts
                .next()
                .unwrap_or_else(|| panic!("linha malformada em CHECKSUMS.lock: {line:?}"));
            let expected_checksum_hex = parts
                .next()
                .unwrap_or_else(|| panic!("linha sem checksum em CHECKSUMS.lock: {line:?}"));
            let version_prefix = filename.split('_').next().unwrap_or(filename);
            let version: i64 = version_prefix.parse().unwrap_or_else(|_| {
                panic!(
                    "nome de migração sem prefixo de versão numérico em CHECKSUMS.lock: {filename:?}"
                )
            });

            LockedMigration {
                filename,
                version,
                expected_checksum_hex,
            }
        })
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn find_by_version(migrations: &[Migration], version: i64) -> Option<&Migration> {
    migrations.iter().find(|m| m.version == version)
}

#[test]
fn migracoes_lancadas_mantem_o_checksum_pinado() {
    let migrator = sqlx::migrate!("./migrations");
    let locked = parse_lock_file();
    assert!(
        !locked.is_empty(),
        "CHECKSUMS.lock está vazio — nenhuma migração lançada está protegida"
    );

    let mut failures = Vec::new();

    for entry in &locked {
        match find_by_version(&migrator.migrations, entry.version) {
            None => failures.push(format!(
                "{} (versão {}): a migração listada em CHECKSUMS.lock não existe mais em \
                 migrations/ — uma migração já lançada não pode ser removida, só recebe uma nova \
                 migração corretiva",
                entry.filename, entry.version
            )),
            Some(migration) => {
                let actual_checksum_hex = to_hex(&migration.checksum);
                if actual_checksum_hex != entry.expected_checksum_hex {
                    failures.push(format!(
                        "{} (versão {}): checksum mudou de {} para {} — uma migração já lançada \
                         não pode mudar, nem no comentário; o sqlx valida esse checksum ao abrir \
                         o banco e recusa bancos que já a aplicaram. Reverta o arquivo e crie uma \
                         migração NOVA para a correção pretendida.",
                        entry.filename,
                        entry.version,
                        entry.expected_checksum_hex,
                        actual_checksum_hex
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "\n\n{} migração(ões) já lançada(s) divergem do checksum pinado:\n\n{}\n",
        failures.len(),
        failures.join("\n\n")
    );
}
