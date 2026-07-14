---
target: linha Reserva após financiar (plano 080)
total_score: 19
p0_count: 0
p1_count: 0
timestamp: 2026-07-14T22-17-46Z
slug: src-screens-scenarios-tsx
---

Method: dual-agent (A: sonnet a461406cf05203e1 · B: haiku add2778dafbe0b2a3)

Escopo: linha "Reserva após financiar" do card "Empréstimo simulado" (ScenarioCompare) — escada 3 faixas + badge antes → depois + popover novo (plano 080).

## Design Health Score (heurísticas aplicáveis ao escopo)

| #                  | Heurística                   | Nota              | Achado                                                                             |
| ------------------ | ---------------------------- | ----------------- | ---------------------------------------------------------------------------------- |
| 1                  | Visibilidade do status       | 3→4               | "Antes" sem sinal visual de estado antigo — CORRIGIDO (span neutro `--text-faint`) |
| 2                  | Sistema × mundo real         | 4                 | Linguagem do método ("Zona amarela", "Paz"); popover fecha o modelo mental         |
| 4                  | Consistência e padrões       | 3→4               | Divergia do padrão `scn-kpi__state-origin` (antes neutro) — CORRIGIDO              |
| 6                  | Reconhecimento > memorização | 4                 | Popover elimina memorização de limiares                                            |
| 8                  | Design minimalista           | 3                 | Popover denso (fórmula + faixas + alvo num bloco de 280px)                         |
| **Total (escopo)** |                              | **17/20 → 19/20** |                                                                                    |

## Anti-Patterns Verdict

Passa. Detector determinístico: exit 0, zero findings (re-verificado no worktree). Sem tells de AI: reuso disciplinado das convenções do arquivo (separador "·", tripla codificação ícone+cor+rótulo, tokens de tema).

## Priority Issues

- **[P1] "Antes" herdava a cor do estado do "depois"** (`ReserveMonthsBadge`) — num cruzamento de faixa o valor antigo parecia da mesma faixa do novo. **CORRIGIDO nesta entrega**: span `.scn-loan-summary__reserve-before` com `--text-faint`, teste de regressão adicionado.
- **[P2] Seta "→" sem alternativa sr-only** — pronúncia varia entre leitores de tela; `PhaseBadge.tsx` já documenta o padrão sr-only. Follow-up.
- **[P2] Popover denso** — body cobre fórmula + 3 faixas + alvo; orçamento do `InfoPopover` é "1–2 frases". Follow-up de copy.
- **[P3] "Paz" usa `--primary-quiet-text`** enquanto as 4 badges "boas" irmãs usam `--success-400` — escolha documentada (precedente TotaisScreen); confirmar intenção.

## O que funciona

- Régua de 3 faixas com fronteira 12,0 inclusiva documentada (divergência deliberada do Termômetro anotada no código).
- Nenhum canal só-cor: ícone aria-hidden + palavra + cor sempre juntos.
- "Label · antes → depois" é reuso de padrões existentes no arquivo, não invenção isolada.
