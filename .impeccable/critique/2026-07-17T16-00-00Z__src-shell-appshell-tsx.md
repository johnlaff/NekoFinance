# Critique — fundação "Midnight Purr" (AppShell + tokens)

- Target: `src/shell/AppShell.tsx` (+ `src/redesign.css` shell, `src/design-system/tokens/*`)
- Provenance: dual sub-agent (Assessment A design review + Assessment B detector/browser), synthesized by the orchestrator.
- Date: 2026-07-17

## Veredito

**Não lê como AI slop.** Malha zinc de baixo croma, dinheiro tabular, separação
marca×status verificada ao vivo (troca de acento não move status), nav de fonte única
(`NAV_ITEMS`). Dock flutuante com blur é tendência 2025-26, mas funcionalmente
justificado (auto-hide libera área útil).

## Heurísticas (A)

visibility 3 → 4 pós-fix · match 4 · control 4 · consistency 4 · error-prevention 4 ·
recognition 2.5 · flexibility 3.5 · aesthetic 4 · error-recovery 3 · help 2.
Score ~34/40.

## Achados e desfecho

| Sev | Achado                                                                                                                                                | Desfecho                                                                                                                     |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| P0  | Regra legada `@media ≤680px` em App.css devolvia o scroll ao documento; auto-hide do dock e coordenação large-title nunca disparavam em telefone real | **Corrigido** (bloco do shell antigo `.ak` removido do media query; regressão e2e adicionada em `foundation-visual.spec.ts`) |
| P1  | Item "Mia" perdia o nome acessível no trilho tablet (rótulo `display:none`; SVG `aria-label="Neko"` vencia)                                           | **Corrigido** (`aria-label` no NavButton e nas tabs do dock; ícones decorativos `aria-hidden`)                               |
| P1  | Alvos de toque no trilho tablet com 34–39px de altura (audit B)                                                                                       | **Corrigido** (`min-height: var(--hit-touch)` no trilho; rodapé alargado)                                                    |
| P1  | Detector: `transition: padding` na tab ativa do dock (animação de layout)                                                                             | **Corrigido** (transição removida)                                                                                           |
| P2  | Trilho tablet ícone-only com pista apenas em hover (breakpoint touch)                                                                                 | Aberto — follow-up das ondas (rótulo curto ou tooltip por toque)                                                             |
| P2  | Acento "Lima" cromaticamente vizinho do verde de status (única paleta que tensiona a regra marca×status)                                              | Aceito conscientemente — paleta herdada da direção selada; revisitar se incomodar no uso real                                |
| P2  | `Disclosure` variante warn usa borda lateral colorida (ban do skill)                                                                                  | Aberto — retrabalho do componente na onda respectiva                                                                         |
| P3  | Bloco "Diagnóstico de animações" (dev-tool) visível em Aparência                                                                                      | Aberto — mover para atrás de um disclosure/tela avançada numa onda                                                           |
| P3  | Área vazia lateral em ≥1920px (max-width 1160)                                                                                                        | Observação para as ondas (testar ultrawide)                                                                                  |

## Evidência (B)

- Contraste: todos os pares do chrome medidos ≥4.5:1 (texto) e ≥3:1 (não-textual), 2 temas.
- Overflow horizontal 390px: nenhum (`scrollWidth` ≤ `innerWidth`, `.sh-body` idem).
- Foco: ordem = DOM/visual, anel visível (`--shadow-focus`) em todas as paradas.
- `data-accent="ambar"`: `--accent` muda; `--success-400`/`--warning-400` imóveis.
- Alvos de toque mobile: todos ≥44px (FAB exatamente 44×44).

## Forças

1. Separação marca×status real e verificada, não só documentada.
2. `NAV_ITEMS` como fonte única de navegação nos 3 chromes.
3. Dinheiro tipograficamente dominante sem cor nem animação.
