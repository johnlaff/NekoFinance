---
timestamp: 2026-07-13T02-05-26Z
slug: src-screens-scenarios-tsx
---
# Critique + audit — ciclo de vida do empréstimo hipotético (feat/loan-lifecycle)

Alvo: src/screens/scenarios.tsx (ScenarioWorkbench, HypotheticalList, LoanGroupItem, LoanSection) + scenarios.css. Duas avaliações isoladas (design review + detector/auditoria técnica), sintetizadas.

## Veredito

Product-slop: **passa** — vocabulário 100% do DS, copy específica do domínio (`loanDeathNote` nomeia o que morre; aviso de restauração antecipa efeito colateral não-óbvio). Detector `detect.mjs`: **0 hits** no diff.

## Scores

- Heurísticas de Nielsen: 1:3 · 2:4 · 3:3 · 4:3 · 5:3 · 6:4 · 7:3 · 8:3 · 9:4 · 10:2 (antes das correções)
- Audit: A11y 2/4 · Perf 3/4 · Theming 4/4 · Responsivo 4/4 · Anti-patterns 3/4 (antes das correções)

## Findings e desfecho

| Sev | Finding | Desfecho |
| --- | --- | --- |
| P1 | Foco perdido ao trocar botões→confirmação de remoção (cai no body numa ação destrutiva) | **Corrigido**: foco programático no bloco `role="alert"` (tabIndex −1); cancelar devolve ao botão "Remover"; teste de `document.activeElement` adicionado |
| P1 | `aria-live` não reanuncia mensagem idêntica consecutiva | **Corrigido**: recibo com `tick` monotônico remonta o `<span>` via key |
| P1 | Accent brass do modo edição inerte (só colore ícone que não existia) | **Corrigido**: ícone `Pencil` passado ao Disclosure + borda brass via `.scn-loan-group--editing` |
| P2 | Remover o empréstimo em edição deixa o formulário órfão | **Corrigido**: "Remover" desabilita durante a edição (como "Editar") |
| P2 | Nested-card: `.scn-loan-group__confirm` com borda+fundo dentro do pill do grupo | **Corrigido**: chassi removido; aviso vira texto em `--danger-400` + ações |
| P2 | Ordem de seções força o Editar a atravessar "Simular alteração" | **Adiado** (decisão de IA pré-existente; auto-scroll mitiga; discutir à parte) |
| P2 | Trocar de alvo de edição descarta alterações em silêncio | **Adiado** (simulação, não dado real; follow-up) |
| P3 | Grupo recém-criado nascia recolhido | **Corrigido**: `defaultOpen={isNew \|\| isEditing}` |
| P3 | "Desembolso" sem camada didática | **Corrigido**: `InfoPopover` ao lado do rótulo (trigger fora do `<label>`) |
| P3 | Flash de 1.44s acima do orçamento 130–480ms | **Mantido** deliberadamente: highlight de localização one-shot, não feedback de interação; colapsa sob reduced-motion |
| P3 | `size="sm"` (28px) menor que a lixeira por linha (36px) | **Mantido**: convenção do arquivo inteiro para botões com texto; ≥ mínimo WCAG 2.5.8 |
| P3 | Agrupamento sem `useMemo` | **Mantido**: lista pequena por natureza (dezenas de linhas) |

Pós-correções, os dois P1s de a11y e o anti-pattern estrutural estão zerados; gates (`npm run check`, vitest, E2E 44/44) verdes.
