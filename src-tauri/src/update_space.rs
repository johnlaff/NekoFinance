//! Pré-checagem de espaço em disco antes de aplicar um update.
//!
//! O instalador do updater precisa de espaço em DOIS lugares: o diretório da
//! instalação (onde o binário novo substitui o antigo) e o diretório temporário
//! (onde o pacote baixado é extraído antes de aplicar). Um disco quase cheio
//! trunca o instalador no meio da escrita — o cenário que motivou o runbook de
//! recuperação de update. Este módulo é o núcleo puro da régua: recebe uma
//! medição já feita (`SpaceProbe`) e devolve um veredito determinístico
//! (`SpaceVerdict`), sem tocar o sistema de arquivos — isso fica na casca de IO
//! em `commands::update_cmds`.
//!
//! A régua usa aritmética inteira (bytes), nunca float, para o veredito ser
//! reproduzível bit a bit entre plataformas.

/// 1 MiB em bytes — unidade base da régua de espaço.
const MIB: u64 = 1024 * 1024;

/// Piso do footprint instalado quando a medição real falha ou o app é pequeno
/// demais para ser um número confiável (ex.: instalação parcial). Abaixo disso
/// a margem de 25% deixaria de proteger contra qualquer update real.
const MIN_INSTALL_FOOTPRINT_BYTES: u64 = 60 * MIB;

/// Piso do espaço temporário exigido, independente do footprint instalado —
/// cobre a extração de um pacote pequeno sem exigir menos que um mínimo seguro.
const MIN_TEMP_NEEDED_BYTES: u64 = 32 * MIB;

/// Medição bruta de disco, já colhida pela casca de IO. Cada campo documenta
/// como ele degrada quando a medição real não é possível — a regra geral é
/// "incerteza vira conservador" (nunca deixa passar um update que pode faltar
/// espaço).
pub struct SpaceProbe {
    /// Bytes livres no volume do diretório de instalação.
    pub install_free_bytes: u64,
    /// Bytes livres no volume do diretório temporário do SO.
    pub temp_free_bytes: u64,
    /// Incerteza na identificação de volume degrada para `true` (conservador):
    /// tratar como o mesmo volume soma as duas exigências contra um único
    /// espaço livre, o cenário mais apertado possível.
    pub same_volume: bool,
    /// Tamanho da instalação atual em disco. `0` = não conseguiu medir; nesse
    /// caso a régua cai no piso (`MIN_INSTALL_FOOTPRINT_BYTES`).
    pub installed_footprint_bytes: u64,
}

/// Veredito da pré-checagem, serializável para o frontend.
#[derive(serde::Serialize)]
pub struct SpaceVerdict {
    pub ok: bool,
    pub required_bytes: u64,
    pub free_bytes: u64,
    pub missing_bytes: u64,
}

/// Aplica a régua de espaço sobre uma medição já colhida.
///
/// A exigência de instalação é o footprint atual (ou o piso de 60 MiB, o que
/// for maior) com 25% de margem — o binário novo convive brevemente com o
/// antigo durante a troca. A exigência de temp é um terço da exigência de
/// instalação (ou o piso de 32 MiB) — o pacote baixado é bem menor que a
/// instalação expandida.
///
/// Quando os dois diretórios estão no mesmo volume (ou a identificação do
/// volume é incerta), as duas exigências competem pelo MESMO espaço livre —
/// somam contra o menor dos dois `free_bytes` medidos (o par deveria reportar
/// o mesmo valor nesse caso; `min` é a defesa caso não reporte). Em volumes
/// distintos, cada exigência é avaliada isoladamente contra o seu próprio
/// volume, então uma falta num lado nunca é escondida por sobra no outro.
pub fn evaluate(probe: SpaceProbe) -> SpaceVerdict {
    let install_footprint = probe
        .installed_footprint_bytes
        .max(MIN_INSTALL_FOOTPRINT_BYTES);
    let install_needed = install_footprint * 5 / 4;
    let temp_needed = (install_needed / 3).max(MIN_TEMP_NEEDED_BYTES);

    if probe.same_volume {
        let required = install_needed + temp_needed;
        let free = probe.install_free_bytes.min(probe.temp_free_bytes);
        let missing = required.saturating_sub(free);
        SpaceVerdict {
            ok: missing == 0,
            required_bytes: required,
            free_bytes: free,
            missing_bytes: missing,
        }
    } else {
        let install_missing = install_needed.saturating_sub(probe.install_free_bytes);
        let temp_missing = temp_needed.saturating_sub(probe.temp_free_bytes);
        let missing = install_missing + temp_missing;
        SpaceVerdict {
            ok: missing == 0,
            required_bytes: install_needed + temp_needed,
            free_bytes: probe.install_free_bytes + probe.temp_free_bytes,
            missing_bytes: missing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Footprint de 200 MiB → install_needed = 250 MiB. temp_needed = install_needed/3 em
    // BYTES (divisão inteira trunca, não é "250 MiB / 3" arredondado para MiB — os dois
    // resultados divergem por causa do truncamento, daí o cálculo explícito aqui).
    const FOOTPRINT_200_MIB: u64 = 200 * MIB;
    const INSTALL_NEEDED_200_MIB: u64 = 250 * MIB;
    const TEMP_NEEDED_200_MIB: u64 = INSTALL_NEEDED_200_MIB / 3;

    #[test]
    fn sobra_espaco_nos_dois_volumes_aprova() {
        let probe = SpaceProbe {
            install_free_bytes: 1024 * MIB,
            temp_free_bytes: 1024 * MIB,
            same_volume: false,
            installed_footprint_bytes: FOOTPRINT_200_MIB,
        };
        let v = evaluate(probe);
        assert!(v.ok);
        assert_eq!(v.missing_bytes, 0);
    }

    #[test]
    fn falta_espaco_so_no_volume_da_instalacao_reprova() {
        // install_needed = 250 MiB; sobra só 100 MiB no volume da instalação.
        let probe = SpaceProbe {
            install_free_bytes: 100 * MIB,
            temp_free_bytes: 1024 * MIB,
            same_volume: false,
            installed_footprint_bytes: FOOTPRINT_200_MIB,
        };
        let v = evaluate(probe);
        assert!(!v.ok);
        assert_eq!(v.missing_bytes, 150 * MIB); // 250 - 100
    }

    #[test]
    fn falta_espaco_so_no_temp_reprova() {
        let probe = SpaceProbe {
            install_free_bytes: 1024 * MIB,
            temp_free_bytes: 10 * MIB,
            same_volume: false,
            installed_footprint_bytes: FOOTPRINT_200_MIB,
        };
        let v = evaluate(probe);
        assert!(!v.ok);
        assert_eq!(v.missing_bytes, TEMP_NEEDED_200_MIB - 10 * MIB);
    }

    // Mesmo volume: cada exigência isolada caberia (150 MiB cobre install OU temp
    // separadamente), mas a SOMA (install + temp) estoura o espaço livre único.
    #[test]
    fn mesmo_volume_soma_as_exigencias() {
        let probe = SpaceProbe {
            install_free_bytes: 150 * MIB,
            temp_free_bytes: 150 * MIB,
            same_volume: true,
            installed_footprint_bytes: FOOTPRINT_200_MIB,
        };
        let v = evaluate(probe);
        assert!(!v.ok, "150 MiB não cobre a soma das duas exigências");
        let required = INSTALL_NEEDED_200_MIB + TEMP_NEEDED_200_MIB;
        assert_eq!(v.required_bytes, required);
        assert_eq!(v.free_bytes, 150 * MIB);
        assert_eq!(v.missing_bytes, required - 150 * MIB);
    }

    // Volumes distintos: cada exigência é medida contra o SEU volume — instalação
    // sobrando não compensa falta no temp, e vice-versa.
    #[test]
    fn volumes_distintos_cada_exigencia_contra_o_seu_volume() {
        // install_needed = 250 MiB, sobra 300 MiB → cobre. temp_needed sobra só 20 MiB → falta.
        let probe = SpaceProbe {
            install_free_bytes: 300 * MIB,
            temp_free_bytes: 20 * MIB,
            same_volume: false,
            installed_footprint_bytes: FOOTPRINT_200_MIB,
        };
        let v = evaluate(probe);
        assert!(!v.ok);
        assert_eq!(v.missing_bytes, TEMP_NEEDED_200_MIB - 20 * MIB);
    }

    // Footprint 0 (medição falhou) cai no piso de 60 MiB → install_needed = 75 MiB;
    // temp_needed = max(75/3, 32) = 32 MiB (25 MiB do cálculo perde para o piso).
    #[test]
    fn footprint_zero_aplica_o_piso() {
        let probe = SpaceProbe {
            install_free_bytes: 1024 * MIB,
            temp_free_bytes: 1024 * MIB,
            same_volume: false,
            installed_footprint_bytes: 0,
        };
        let v = evaluate(probe);
        assert!(v.ok);
        assert_eq!(v.required_bytes, 75 * MIB + 32 * MIB);
    }
}
