//! Conciliação avançada: merge de 3 vias por campo. Núcleo PURO, sem IO.
//!
//! `base` = valor do campo como foi importado da planilha da última vez. `local` = valor atual no
//! app (pode ter sido editado). `sheet` = valor agora na planilha. A decisão preserva a edição
//! local quando só o local mudou, aplica a planilha quando só ela mudou, e levanta conflito quando
//! ambos divergem do base — abrindo o gate humano em vez de sobrescrever em silêncio.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeDecision {
    /// Gravar o valor da planilha (e semear/alinhar o base).
    ApplySheet,
    /// Manter o valor local (edição preservada, ou nada mudou).
    KeepLocal,
    /// Ambos mudaram desde o base → gate humano; não sobrescrever.
    Conflict,
}

/// Merge de 3 vias de um campo. `base` ausente = primeiro import (sem snapshot) → aplica a planilha.
pub fn reconcile<T: Eq>(base: Option<&T>, local: &T, sheet: &T) -> MergeDecision {
    // Convergência: se local e planilha já concordam, não há o que resolver (nem conflito).
    if local == sheet {
        return MergeDecision::KeepLocal;
    }
    match base {
        None => MergeDecision::ApplySheet,
        Some(b) => {
            let local_changed = local != b;
            let sheet_changed = sheet != b;
            match (local_changed, sheet_changed) {
                (false, true) => MergeDecision::ApplySheet, // só a planilha
                (true, false) => MergeDecision::KeepLocal,  // só o local → preserva edição
                (true, true) => MergeDecision::Conflict,    // ambos divergiram, e diferem
                (false, false) => MergeDecision::KeepLocal, // inalcançável (local==sheet acima)
            }
        }
    }
}

/// Resultado da conciliação de um campo: o que gravar, o novo base (snapshot) e se há conflito.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldOutcome<T> {
    /// Valor a gravar no `transaction`.
    pub value: T,
    /// Novo `source_*` (base) a persistir.
    pub source: T,
    /// `true` → registrar conflito; não sobrescrever o valor local.
    pub conflict: bool,
}

/// Resolve um campo e devolve o que gravar. `ApplySheet`: planilha vence, base = planilha.
/// `KeepLocal`: mantém o local, base realinha à planilha (não havia conflito). `Conflict`:
/// preserva o local, base permanece o antigo (conflito persiste até resolução).
pub fn apply<T: Eq + Clone>(base: Option<&T>, local: &T, sheet: &T) -> FieldOutcome<T> {
    match reconcile(base, local, sheet) {
        MergeDecision::ApplySheet => FieldOutcome {
            value: sheet.clone(),
            source: sheet.clone(),
            conflict: false,
        },
        MergeDecision::KeepLocal => FieldOutcome {
            value: local.clone(),
            source: sheet.clone(),
            conflict: false,
        },
        MergeDecision::Conflict => FieldOutcome {
            value: local.clone(),
            source: base.cloned().unwrap_or_else(|| local.clone()),
            conflict: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_keeps_local_edit_and_realigns_base() {
        // Só o local mudou (planilha == base) → grava o local, base segue a planilha, sem conflito.
        let o = apply(Some(&10), &15, &10);
        assert_eq!(
            o,
            FieldOutcome {
                value: 15,
                source: 10,
                conflict: false
            }
        );
    }

    #[test]
    fn apply_sheet_update_overwrites() {
        let o = apply(Some(&10), &10, &20);
        assert_eq!(
            o,
            FieldOutcome {
                value: 20,
                source: 20,
                conflict: false
            }
        );
    }

    #[test]
    fn apply_conflict_preserves_local_and_keeps_base() {
        // Ambos mudaram → não sobrescreve (value=local), base intacto, conflito sinalizado.
        let o = apply(Some(&10), &15, &20);
        assert_eq!(
            o,
            FieldOutcome {
                value: 15,
                source: 10,
                conflict: true
            }
        );
    }

    #[test]
    fn first_import_applies_sheet() {
        // Sem base e divergindo do que está no app → a planilha semeia o valor.
        assert_eq!(reconcile(None, &10, &99), MergeDecision::ApplySheet);
        // Sem base mas já igual à planilha → nada a gravar.
        assert_eq!(reconcile(None, &10, &10), MergeDecision::KeepLocal);
    }

    #[test]
    fn nothing_changed_keeps_local() {
        assert_eq!(reconcile(Some(&10), &10, &10), MergeDecision::KeepLocal);
    }

    #[test]
    fn only_sheet_changed_applies_sheet() {
        assert_eq!(reconcile(Some(&10), &10, &20), MergeDecision::ApplySheet);
    }

    #[test]
    fn only_local_changed_keeps_local() {
        assert_eq!(reconcile(Some(&10), &15, &10), MergeDecision::KeepLocal);
    }

    #[test]
    fn both_changed_is_conflict() {
        assert_eq!(reconcile(Some(&10), &15, &20), MergeDecision::Conflict);
    }

    #[test]
    fn both_changed_to_same_value_is_not_conflict() {
        // Convergência: local e planilha foram para o MESMO valor → sem conflito.
        assert_eq!(reconcile(Some(&10), &20, &20), MergeDecision::KeepLocal);
    }

    #[test]
    fn works_for_strings() {
        let base = "Mercado".to_string();
        let local = "Mercado e farmácia".to_string();
        let sheet = "Mercado".to_string();
        // Descrição editada localmente, planilha intacta → preserva a edição.
        assert_eq!(
            reconcile(Some(&base), &local, &sheet),
            MergeDecision::KeepLocal
        );
    }
}
