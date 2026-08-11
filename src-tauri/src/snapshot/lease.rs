//! O árbitro do lease: quem pode publicar o snapshot no Drive, decidido por SEQUÊNCIA — nunca por
//! `created_at`. Relógios de dois aparelhos divergem (fuso, deriva, hora errada); uma sequência
//! monotônica por aparelho não. Esta é a única função com a regra de posse — shell e UI a
//! consultam e obedecem, nunca reimplementam a comparação.

use super::manifest::SnapshotManifest;

/// O veredito fechado do árbitro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseVerdict {
    /// Local e remoto seguem na mesma base — nada para subir nem puxar.
    UpToDate,
    /// O remoto avançou e o local não — baixar antes de qualquer gesto.
    Pull,
    /// O local avançou e o remoto não (inclusive quando nenhum snapshot foi publicado ainda) —
    /// publicar é seguro.
    Push,
    /// Os dois avançaram a partir da mesma base — resolução manual, nunca merge automático.
    Conflict,
}

/// Decide o veredito comparando a sequência LOCAL (o que este aparelho quer publicar), a
/// sequência BASE (a última que este aparelho sincronizou) e o manifest remoto — `None` quando
/// nenhum snapshot foi publicado ainda, tratado como sequência remota zero.
///
/// Função TOTAL: mesmo uma entrada que não deveria surgir na prática (`local < base`, um estado
/// remoto que regrediu) cai em "não avançou" em vez de entrar em pânico — o árbitro nunca falha,
/// só devolve o veredito que a comparação de sequências sustenta.
pub fn decide(local: i64, base: i64, remote: Option<&SnapshotManifest>) -> LeaseVerdict {
    let remote_seq = remote.map(|m| m.sequence).unwrap_or(0);
    let local_advanced = local > base;
    let remote_advanced = remote_seq > base;
    match (local_advanced, remote_advanced) {
        (false, false) => LeaseVerdict::UpToDate,
        (false, true) => LeaseVerdict::Pull,
        (true, false) => LeaseVerdict::Push,
        (true, true) => LeaseVerdict::Conflict,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(device_id: &str, sequence: i64) -> SnapshotManifest {
        SnapshotManifest {
            device_id: device_id.to_string(),
            sequence,
            created_at: "2026-08-11T12:00:00Z".to_string(),
            app_version: "0.2.1".to_string(),
            schema_version: 42,
        }
    }

    #[test]
    fn lease_verdict_covers_every_combination_of_local_base_remote() {
        for (label, local, base, remote, expected) in [
            // Primeira subida: nunca publicou, tem mudança local para subir.
            ("primeira subida", 1, 0, None, LeaseVerdict::Push),
            // Remoto ausente e nada local: nenhum snapshot em lugar nenhum ainda.
            (
                "remoto ausente, nada local",
                0,
                0,
                None,
                LeaseVerdict::UpToDate,
            ),
            // Empate exato: publicou e nada mudou desde então (o próprio remoto na base).
            (
                "empate exato",
                5,
                5,
                Some(manifest("outro-aparelho", 5)),
                LeaseVerdict::UpToDate,
            ),
            // Avanço unilateral local: subir é seguro.
            (
                "avanço unilateral local",
                6,
                5,
                Some(manifest("outro-aparelho", 5)),
                LeaseVerdict::Push,
            ),
            // Avanço unilateral remoto: puxar antes de qualquer gesto.
            (
                "avanço unilateral remoto",
                5,
                5,
                Some(manifest("outro-aparelho", 6)),
                LeaseVerdict::Pull,
            ),
            // Divergência dupla: os dois avançaram a partir da MESMA base — conflito, mesmo que
            // o remoto tenha avançado mais que o local (a magnitude não desempata sozinha).
            (
                "divergência dupla",
                6,
                5,
                Some(manifest("outro-aparelho", 7)),
                LeaseVerdict::Conflict,
            ),
            // device_id do remoto nunca entra na conta — só a sequência decide posse.
            (
                "device_id do remoto é irrelevante ao veredito",
                5,
                5,
                Some(manifest("device-qualquer", 5)),
                LeaseVerdict::UpToDate,
            ),
        ] {
            assert_eq!(
                decide(local, base, remote.as_ref()),
                expected,
                "caso: {label}"
            );
        }
    }

    #[test]
    fn local_behind_base_is_treated_as_not_advanced_never_panics() {
        // Não deveria surgir na prática (base é sempre <= local), mas o árbitro é total.
        assert_eq!(decide(3, 5, None), LeaseVerdict::UpToDate);
        assert_eq!(decide(3, 5, Some(&manifest("d", 6))), LeaseVerdict::Pull);
    }
}
