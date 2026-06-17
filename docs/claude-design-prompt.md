# Claude Design Prompt

Use this prompt in Claude Design or a design-focused Claude session. It assumes the repo already has an initial design system and asks Claude Design to refine it into a production-ready, domain-specific system.

Updated 2026-06-13 after a source-neutral design review: the design system must support spreadsheet-compatible edge cases (cell notes as ledger detail, deliberate savings distinct from realized cushion, credit-led spending, reimbursement/pass-through rows), the deeper method (annual savings average, predictability/coverage, cash ≠ performance), an adaptation-coaching layer, the desired motion direction (animated, fluid, deliberate reveals), and a hard product lesson: the dashboard accreted bolt-on cards and now needs a coherent information architecture, not more cards.

Earlier pass (2026-06-12): repo/spec analysis, public example workbook structure, source-neutral methodology synthesis, 2026 market/state-of-the-art design-system research.

Do not attach raw private methodology/source files to Claude Design. If methodology input is needed, provide only a source-neutral summary of rules, workflows, examples, and edge cases.

```text
You are a senior product designer and design systems lead updating an existing design system for Neko Finance.

This is not a greenfield branding exercise. Your job is to evolve, harden, and reconcile the existing “Midnight Ledger” design system with the actual product, specs, implementation, spreadsheet methodology, and privacy model.

<context>
Product: Neko Finance.
Type: local-first Tauri desktop app, React 19 + TypeScript, SQLite, Google Sheets/local XLSX import, future Open Finance/OFX/CSV import.
Primary user: one solo software/AI engineer managing personal finances at night in a low-light home office.
Primary workflow: daily check-in under 30 seconds, forecast-first dashboard, spreadsheet import/reconciliation, credit/invoice tracking, reserve/liquidity planning, and Mia as a calm financial copilot.
Language/locale: PT-BR UI copy, BRL currency, Brazilian financial concepts.
Brand: precisa, discreta, confiável, amigável.
Anti-references: traditional banks, gamified apps, corporate ERPs, neon neobanks, generic purple AI dashboards, childish mascots.
Current design direction: dark-first “Midnight Ledger”: graphite background, jade primary, brass warmth, Hanken Grotesk UI, Geist Mono money, Newsreader only for methodology/editorial content.
Motion direction: the product should feel animated and fluid, with deliberate reveals. The app already ships a circular-reveal theme switch via the View Transitions API; treat that as the motion signature to extend, not a one-off. Motion is chrome over a calm dark surface; it never animates a critical money value into existence and always has a `prefers-reduced-motion` fallback. The goal is a reading surface that feels alive, not a dashboard that twitches.
</context>

<research_basis>
Use these public, non-private reference points as the 2026 quality baseline:
- WCAG 2.2 Recommendation: focus not obscured, target size, redundant entry, accessible authentication, status messages, and error prevention for legal/financial/data workflows.
- WAI-ARIA Authoring Practices: keyboard behavior and roles for tabs, dialogs, menus, comboboxes, tables/grids, tooltips, and disclosure patterns.
- W3C Design Tokens Community Group: tokens as a cross-tool contract. Treat the 2025.10 format as a useful preview, not as an authoritative dependency to implement blindly.
- Modern CSS: CSS custom properties, cascade layers, container queries, logical properties, `color-scheme`, `:has()`, `color-mix()`, OKLCH where safe, `forced-colors`, and sRGB fallbacks. Validate against the Tauri target WebView, not only evergreen Chrome.
- React 19: design components can use simpler ref contracts, form/action pending states, and `useFormStatus`/`useActionState` where appropriate. Optimistic UI is allowed for draft proposals, never for final financial writes.
- Headless accessible primitives: prefer native HTML first; use React Aria/Radix/Ark-style primitives for complex widgets where keyboard and screen-reader behavior is easy to get wrong.
- Human-AI UX: disclose capabilities and limits, show provenance, expose uncertainty/review states, support correction/recovery, and keep the user in control.
- Local-first UX: no spinners for local data, network is optional, data location is visible, sync freshness is legible, and the user retains ultimate ownership/control.
- Data visualization: exact values remain available through labels/tables; charts show trend/pressure only and never carry critical meaning through color or hover alone.
</research_basis>

<repo_findings>
The current design system is directionally right but too generic in examples and partially disconnected from production code.

Problems to fix:
- Generated DS examples use English/USD/generic merchants.
- Live app/specs require PT-BR/BRL and domain-specific labels: saldo projetado, Diário hoje, Crédito no mês, Reserva, Régua 1, Régua 2, Fatura, Economia, Caixa de revisão.
- There are two component worlds: generated JSX/global bundle and smaller production TSX components.
- Rich DS concepts exist but are underused or not production-ready: OwnerChip, TransactionRow, ApprovalDiffCard, ChatBubble, Citation, HealthBadge, chart language.
- Some app surfaces reimplement cards, tables, filters, settings panels, import states, and pockets forms outside the DS.
- The design system must become an implementation contract, not just a visual kit.
- `src/design-system/readme.md` still reads like a greenfield proposal, but production already has TSX screens and tested components.
- Production has honest current/future separation: Dashboard, Transações, Configurações/import, Metodologia, and a Mia placeholder exist; full chat, write-back approval flow, Crédito/Fatura, Caixa de revisão, and advanced reconciliation are future/partial.
- Current tokens cover brand, text, surfaces, money, owners, motion, spacing, elevation, and charts, but not enough semantic state tokens for review/confidence, source cells, checksum conflicts, rollback, privacy, connection lifecycle, liquidity, invoice pressure, or density.
- Some production components still use inline styles or screen-local classes; the updated DS should define migration targets, not demand a big-bang rewrite.
- Dashboard IA lesson (the central UX problem to solve): the Dashboard accreted feature cards one at a time (forecast hero, safe-to-spend, deficit notice, a predictability/coverage card, an adaptation/“colchão” card, a four-tile metric grid, a per-month performance strip, the daily projection table, a Mia placeholder, a pockets card). Each was correct in isolation; stacked, they degraded into an incoherent vertical pile with no information hierarchy. The redesign’s first job is a real Dashboard IA — decide what is the single hero, what is secondary, what collapses behind disclosure, what moves to a dedicated screen — not to style more cards. Card stacking is the failure mode here; reach for sectioning, progressive disclosure, and a deliberate reading order instead.
- Production concepts that need first-class DS contracts: a predictability/coverage indicator (per future month: how much of typical spend is already entered, “trusted through month X”, what is missing), a “pode gastar hoje” guardrail that is the tighter of cash floor and annual savings, a cash-vs-performance distinction surfaced per month, and an adaptation-coaching surface (see the adaptation_coaching section). These currently live as bespoke cards with ad-hoc classes; the DS must give them stable, accessible, motion-aware contracts.
</repo_findings>

<current_repo_snapshot>
Implemented or partially implemented surfaces:
- Dashboard forecast view: projected balance, safe-to-spend today, deficit warning, daily projection table, reserve months, credit monthly metric, pockets card.
- Transactions screen: PT-BR transaction list, filters/search, BRL formatting.
- Settings/privacy: Google Sheets/local XLSX import and privacy/data-location surfaces.
- Methodology screen: source-neutral explanation of forecast-first method.
- Mia screen: honest placeholder; no fake chat/composer yet.
- Pockets/liquidity: partial support for liquid, reserve, restricted, illiquid, net worth.

Future or not-yet-production surfaces:
- Dedicated Crédito/Fatura module.
- Caixa de revisão for ambiguous imports and reconciliation.
- ApprovalDiffCard/write-back/rollback/checksum flows.
- Full Mia chat with deterministic tool citations and approval proposals.
- Scenario/sandbox builder for purchase, installment, debt, reserve, moving/travel/property, income-loss, and household/business cases.

Design implication: every deliverable must label whether it targets current production, near-term implementation, or future reference. Do not fake future AI/write-back functionality as live product.
</current_repo_snapshot>

<domain_model>
Important concepts:
- Person, Profile, Device.
- Account, Transaction, Split, Payment Method, Category.
- Daily Budget, Daily Check-in.
- Régua 1 / Diário: debit, PIX, and cash daily spend against daily budget.
- Régua 2 / Fatura: credit card spending and invoice pressure against income.
- Reserve: first-class emergency fund metric with target months, current months, and trend.
- Liquidity pockets: liquid, restricted, illiquid, reserve, and other account classes that change projection meaning.
- Sheet Mapping and Sync Log.
- System of record is phased: during the current import-only phase the spreadsheet is the system of record and SQLite is the local mirror + enrichment layer; SQLite becomes the system of record only in the future gated bidirectional write-back phase (see `docs/adr/0003`).
- All material Google Sheets writes require structured diff, validation, checksum verification, explicit human approval, and rollback visibility.
</domain_model>

<source_neutral_methodology>
Use the methodology only as source-neutral product behavior. Do not name or quote any private source, course, teacher, institution, file, lesson, video, transcript, URL, email, or personal example.

Method principles the UI must embody:
- The app exists to preserve financial discipline, not to maximize dashboard decoration.
- Forecast is the hero: the user needs to know what will happen by day and by month end, not just what happened yesterday.
- Every transaction should answer: “does this make a future day go negative?”
- The spreadsheet is an engine: formulas, notes, monthly geometry, year blocks, and manually chosen dates are meaningful and must be preserved.
- The current account balance is only a transit layer. Reserve, investments, restricted balances, and assets are separate wealth/liquidity layers.
- Daily spending is intentionally simple: one daily variable-spend number, not category budgeting for every category.
- Categories are diagnostic after the fact; they are not the main forward-planning instrument.
- The primary behavioral split is fixed lifestyle costs vs flexible daily spending vs credit obligations vs deliberate savings/reserve.
- Debit/PIX/cash and credit card spend are tracked independently to avoid self-deception.
- Credit purchases do not enter Diário; they accumulate into invoice/fatura logic and affect forecast at the right payment date.
- Future salary can be partially “already spent” by card bills and installments; make that pressure visible without shame.
- Future manual projections in the spreadsheet are the user’s intent and must not be duplicated by app-generated projections.
- Reserve health is expressed in months of coverage and trend, not only currency balance.
- Savings/economy is a deliberate outflow/allocation, not an automatic leftover. Show “you can save up to X” as a suggestion, not as an auto-write.
- Scenarios are modeled in copies/sandboxes before changing the real plan.
- Use conservative assumptions: uncertain income is not over-counted, recurring costs are not hidden, and “I will spend less later” is not accepted as an invisible fix.
- Education, housing, subscriptions, insurance, investments, reimbursements, taxes, business/personal transfers, and other recurring obligations are represented as generic fixed/planned commitments without exposing private source documents.
- Ambiguous imports go to review; the app must not guess silently.
- A correction that touches the spreadsheet is a proposal, not an automatic action.
- A red forecast is a signal for action, not a moral failure. A green forecast can still be unhealthy if the savings/reserve rate is structurally too low.
- Do not encourage cutting risk protections such as health, housing, insurance, or other high-consequence commitments casually.
- Income increases should default to protecting/saving first, then intentional lifestyle expansion.
- The savings target (20–30% of income) is an ANNUAL average, not a monthly pass/fail. Some months are above, some below; what matters is the year. UI must never shame a single weak month as “below target”; it judges the year-to-date / annual position and treats monthly variance as normal.
- Predictability is the core of the method: the user looks at the present and the future at the same time. A future left empty produces a falsely optimistic projection (the balance climbs as if the user stops eating). Surfacing “how complete is each future month, and what is still missing to make it real” is a first-class job, and a differentiator no mainstream app does.
- Cash is not performance. The running balance can grow on an accumulated buffer while a given month’s performance (income minus outflow) is thin or negative. Show both, and never let a healthy cash balance hide a structurally weak savings rate.
- Cost of living = total outflow minus deliberate savings/investment. The emergency reserve is expressed as cost-of-living × N months (commonly a 12-month “at peace” target), and is a manual decision gate for big purchases, not an automatic block. Reserve coverage should be derived (reserve ÷ monthly cost), not a static number.
- Credit hijacks future salary: today’s card spend becomes a fixed outflow at the invoice due date, so a future month’s performance can already be negative before it arrives. Make that pressure visible without shame, and never double-count it (the lump lands once, at the due date).
- A correct daily number is the tighter of two limits: cash floor (won’t drive any future day below the reserve) and the annual savings room. Present “you can spend up to X today” as the honest minimum of both, with the binding reason stated.

Source-neutral methodology domains the design must support:
- Onboarding from real current cash, known income dates, fixed bills, current card bills, future installments, reserve amount, target reserve months, and daily-spend baseline.
- Daily check-in that reconciles app balance with real account balance, then updates income, fixed outflows, daily spending, savings, and card spend.
- Upfront vs installment purchase decisions, shown as forecast scenarios with reserve impact, invoice impact, monthly pressure, and recommended review gates.
- Credit-to-debit migration, shown as a gradual transition plan rather than a moral judgment.
- Living below income without suppressing quality of life, shown as spending-room and trade-off views, not deprivation visuals.
- Couple/household finances, shown through payer, beneficiary, responsible owner, shared expense, reimbursement, settlement states, autonomy, and shared visibility without surveillance.
- Self-employed or mixed business/personal finances, shown through separate ledgers, owner pay as business outflow/personal inflow, transfer classification, tax/reserve buckets, and cashflow seasonality.
- Debt recovery, shown as stabilization, exact shortfall date/amount, root-cause lever, payoff plan, interest pressure, due-date risk, and progress without shame or gamification.
- Reserve creation, use, and replenishment, shown as months of protection, safe drawdown, replenishment plan, and warning thresholds.
- Post-reserve wealth building, shown as allocation discipline, liquidity class, long-term bucket, and “do not spend” separation.
- Spreadsheet onboarding and evaluation, shown as guided setup, missing-field checks, formula integrity checks, and safe remediation proposals.
- Community/question-derived edge cases, shown only as anonymized patterns and reusable decision rules.

Methodology support-answer pattern:
- Diagnose when and how much money is missing.
- Identify the lever: income, fixed cost, daily spending, credit/fatura, loan structure, reserve use, reimbursement timing, or transfer classification.
- Show the exact app/spreadsheet action.
- Suggest the next check-in or review gate.
</source_neutral_methodology>

<adaptation_coaching>
The user is adapting their real habits to the method and will diverge from it in legitimate ways. The app’s job is to detect those gaps from the data and coach toward the method, never to scold. This is a first-class product surface, not a tooltip.

Core principle: recognize before you teach. Name what the user already does (and validate it) before suggesting the next step. The copilot never says “you should” without first saying “you already do X.”

Named adaptation period with three phases the UI makes explicit, so the learning curve is expected and visible:
- Map: import and see the real picture; expect gaps.
- Calibrate: fill the future, classify income, surface the gaps; numbers stabilize.
- Operate: the daily check-in and the guardrails run cleanly.
Each gap belongs to a phase; show the user where they are and what closes the gap.

Worked gap the design must support (the “cushion” case): the user keeps the surplus from good months as a cash buffer to cover bad months, instead of moving money into a formal savings line and later withdrawing it. This is a valid adaptation. The surface shows, side by side, “registered savings: R$ 0” (muted) and “cushion built this year (realized): R$ X · Y%” (primary), states plainly that the user saves via the cushion rather than a formal line, and offers the next step (formalize savings + a named reserve) as an opt-in, calm invitation — “next level, when you want” — not a correction. Negative-buffer months read as “the cushion did its job,” not as failure.

Other gaps detected the same way: credit-first (no daily speedometer) → teach the invoice/credit-pressure view; future months under-filled → the coverage indicator plus a concrete “enter X, Y, Z to make this month real” checklist; pass-throughs inflating cost of living → the owner/net affordance; untyped variable income → a one-time classification.

Market state of the art to draw from (source-neutral): structural friction that won’t let a period “close” with method steps unreconciled (YNAB-style); proactive, opt-in-tone intervention before damage with a chosen voice (honest vs gentle) (Cleo-style); and an explicitly named, expectation-setting adaptation period. No competitor combines all three; doing so is the defensible difference. Tone is always calm guidance: a red/negative month is a GPS signal, not a moral verdict; progress is shown without gamification (no streaks, confetti, badges).
</adaptation_coaching>

<motion_and_wow>
The product should feel animated and fluid, with deliberate reveal moments. Treat motion as part of the build, designed up front, not a final polish pass.

Signature: the app already does a circular-reveal theme switch via the View Transitions API. Extend that language. Good places for expressive, content-aware motion: theme/screen transitions (circular or directional reveals), the daily check-in confirmation, a forecast/coverage value resolving, staggered entrance of a list’s rows, a lump expanding into its noted sub-items, the cushion/adaptation card revealing its breakdown.

Non-negotiable guardrails (this is a finance reading surface):
- Data is king, motion is chrome. Never animate a critical money figure into existence in a way that delays the user reading it; the value is present and correct first, motion decorates the arrival.
- Every animation has a `prefers-reduced-motion: reduce` alternative (crossfade or instant). The circular reveal degrades to a clean theme swap.
- Ease out with exponential curves; no bounce, no elastic, nothing playful-gamified (that hits the anti-reference wall).
- Stagger is per-content and purposeful, not one uniform entrance bolted onto every section. The reveal should fit what it reveals.
- Reveals enhance an already-visible default; never gate finance content behind a class-triggered transition that could fail to fire (hidden tab, headless render) and ship blank.
- Premium materials are allowed where they stay smooth and serve meaning: blur/backdrop-filter, clip-path/mask for reveals, soft glow on a focused/positive state. Use View Transitions and a real motion library (e.g. Motion/GSAP) rather than hand-rolled keyframes for anything non-trivial; validate performance on the Tauri WebView.

Deliverable: a motion system in the token set (durations, easing curves, named transition presets like theme-reveal, row-stagger, value-resolve, lump-expand), each with its reduced-motion fallback, and guidance on where motion is encouraged vs where it is forbidden (never on a number the user is trying to read, never as decorative idle loops on finance-critical surfaces).
</motion_and_wow>

<spreadsheet_context>
The public example workbook is formula-heavy, not a clean transaction database.

Observed structure:
- Sheets: 2025, 2026, Economia.
- Labels include months in Portuguese: JANEIRO, FEVEREIRO, MARÇO, etc.
- Year sheets use month blocks across the sheet. Daily columns include: Data, Entrada, Saída, Diário, Saldo.
- Summary concepts include: ENTRADAS, SAÍDAS, DIÁRIO, Saída Total, Performance, Economia, TOTAL.
- The workbook contains hundreds of formulas and comments/notes in year sheets. The UI must respect spreadsheet geometry, source cells, notes, formulas, and reconciliation.
- `Saldo` is an encadeado/chained formula column and must never be overwritten by write-back.
- `Data` is structural and must never be overwritten by write-back.
- `Diário` cells can contain meaningful notes/budget context; automatic import should not treat them as generic editable note fields.
- The `Economia` sheet uses year blocks with `mês | Entradas | Economia | %`. `Entradas` and `%` are formula/literal history fields; write-back targets only the correct `Economia` cell after validation.
- The app must distinguish imported/realized data, manual projections from the spreadsheet, app-generated projections, reconciled lumps, shadowed items, and review-required conflicts.
- Never design as if the app owns a perfect normalized source from day one.

Public-safe compatibility scenarios:
- Cell notes may carry ledger detail that is richer than the cell value. The DS must show notes, let the user read/expand a lump into noted sub-items, and never reduce an entry to a generic “Saída <month>” label. Design a TransactionRow / lump-expander that surfaces noted sub-items.
- The savings (`Economia`) column can be 0 while realized cash cushion grows elsewhere. Design must not read `Economia = 0` as “saves nothing”; it is an adaptation state to coach (see adaptation_coaching), with the cushion measured from realized net.
- The daily-variable (`Diário`) column can be 0 when the user routes daily spending through credit and settles it as an invoice lump. Don’t show `Diário hoje R$ 0` as a measured metric in that case; show it as not-applicable / credit-first, and lean into a credit/invoice-pressure view instead.
- Reimbursements and pass-throughs can appear as matching inflow/outflow rows. They are net-zero on performance but can inflate gross inflow/outflow and the cost-of-living base. The DS needs an owner/pass-through affordance (a chip + an expandable net view) so the user can see “gross vs net” without surveillance framing.
- Income can be variable: a stable base plus occasional extras that swing monthly totals. Design conservative-income patterns: forecast on the base, show the extra as a separate “if it arrives” layer, and let the user classify an extra once.
- Future months can be under-filled (for example, salary and fixed bills are present while card invoice or variable spend are missing), which makes projection optimistic. The DS must make “this future month is incomplete” legible (coverage indicator), and keep the realized figure as the honest one.
</spreadsheet_context>

<privacy_policy>
Public design-system docs and examples must be source-neutral and data-free.

Forbidden in output:
- private names, source names, school names, course names, lesson names, author names, emails, handles, domains, URLs, screenshots, transcript quotes, raw attachments, embeddings, OAuth state, API keys, tokens, realistic personal finance data, or personal spreadsheet rows.

Allowed in output:
- generic schemas, synthetic examples, anonymized rule IDs, source-neutral workflows, and public-safe architecture/design notes.

Use generic demo labels such as “Pessoa A”, “Pessoa B”, “Conta principal”, “Cartão principal”, “Instituição”, “Compromisso fixo”, “Documento de origem”, and “Reembolso previsto”.
</privacy_policy>

<state_of_the_art_2026>
Follow current best practice:
- WCAG 2.2 AA minimum, with visible focus that is not obscured, non-color-only statuses, keyboard-first tables/grids, accessible dialogs, target size, reduced motion, accessible authentication, redundant-entry reduction, and prevention/reversal for financial/data-entry errors.
- Use design tokens as an interoperable contract: primitive/global tokens, semantic aliases, component tokens, theme tokens, density tokens, chart tokens, state tokens, and accessibility-mode tokens. Include descriptions, usage rules, contrast intent, and deprecation guidance.
- Prefer CSS custom properties, cascade layers, container queries, logical properties, color-scheme, `:has()`, `color-mix()`/OKLCH where useful, and sRGB fallbacks. Validate against Tauri WebView support.
- Component strategy should align with native-first/headless accessible primitives: native HTML where sufficient; React Aria/Base UI/Radix/Ark-style behavior for complex controls; Neko-specific visual layer over deterministic behavior.
- Data visualization must be auditable: stable colors, direct labels, table fallback, exact values, no decorative chart junk, no 3D, no color-only meaning, no hover-only critical facts.
- AI UX must show provenance, citations, capabilities/limits, uncertainty/review states, deterministic tool output, before/after diffs, rollback, and explicit approval.
- Local-first UX must make device/local DB, offline readiness, sync freshness, export/backup, connection lifecycle, and “data leaves this device” moments visible without alarmism.
- React 19 optimistic/pending patterns may make interactions feel faster, but final finance state must remain conservative: proposals can be optimistic; writes, approvals, and reconciliation are not final until validated.
- Motion/transitions: the View Transitions API (already used for the circular-reveal theme switch) plus a real motion library are the current baseline for fluid, content-aware transitions; always paired with `prefers-reduced-motion` fallbacks and validated on the Tauri WebView. Expressive but never gamified.
- Method-adherence coaching is its own state of the art: structural friction (YNAB), proactive opt-in-tone intervention (Cleo), and a named, expectation-setting adaptation period. Combine all three; keep it calm, recognition-first, and gamification-free.
</state_of_the_art_2026>

<task>
Update the design system so it becomes specific to Neko Finance and production-ready.

Do not create a generic fintech dashboard.
Do not restart the visual identity unless absolutely necessary.
Keep Midnight Ledger if it still fits, but make it sharper, more Brazilian, more spreadsheet-aware, more methodology-aware, more local-first, and less Claude-generic.
</task>

<deliverables>
Provide:

1. Design thesis
Explain the refined design philosophy in 5-7 principles. It must connect to privacy, nighttime use, forecast-first finance, spreadsheet reconciliation, source-neutral methodology, and human-approved AI.

2. Token architecture
Define global, semantic, and component token layers for:
- color
- typography
- spacing
- radius
- elevation
- motion
- density
- data visualization
- owner/person roles
- confidence/review states
- diff/write-back states
- local/privacy/connection states
- liquidity/pocket states
- invoice/credit pressure states
- forecast/manual/app/reconciled data provenance states
Include dark, light, and high-contrast/accessibility variants.

3. Component inventory
Specify production-ready React/TypeScript component contracts for:
- AppShell
- Button
- Input
- Select
- Switch
- SegmentedControl
- Badge
- Card/Panel
- Money
- DataTable
- MetricTile
- ForecastHero
- SafeToSpendCallout (the “pode gastar hoje” guardrail: shows the honest minimum of cash floor and annual-savings room, states the binding reason)
- DeficitNotice
- PredictabilityCard / CoverageIndicator (per future month: % of typical spend entered, “trusted through month X”, total still missing, the “enter X/Y/Z” checklist)
- CashVsPerformanceStrip (per month: performance value + savings rate, with incomplete months marked optimistic, never shamed)
- AdaptationCard / CushionCallout (registered savings vs realized cushion, the recognize-then-teach copy, opt-in next step)
- AdaptationPhaseBadge (Map / Calibrate / Operate)
- LumpBreakdown / NotedSubItems (expand one entry into its noted per-person, per-item detail)
- DailyProjectionTable
- DailyCheckInSpeedometer
- ReguaCard
- CreditPressureCard
- ReserveCard
- PocketCard
- EconomiaCard
- InvoiceCard
- ScenarioCard
- ScenarioBuilder
- TransactionRow
- OwnerChip
- SourceCellBadge
- ConfidenceMeter
- ReviewQueueItem
- ReviewQueue
- SheetMappingPanel
- ApprovalDiffCard
- SyncStatus
- ConnectionStateBanner
- PrivacyDataLocationCard
- MiaChatBubble
- MiaCitation
- ToolResultBlock
- EmptyState
- ErrorState
- Loading/Skeleton states

4. Required screens
Describe implementation-ready layouts for:
- Dashboard forecast view. PRIORITY: this screen currently fails as a stack of bolt-on cards. Deliver a real information architecture first — name the single hero (the forecast/“pode gastar” reading), the secondary band, what collapses behind disclosure (predictability detail, adaptation/cushion coaching, per-month performance), and what graduates to its own screen. Specify the reading order and the responsive behavior. Do not solve it by restyling more cards.
- Transactions/import review.
- Caixa de revisão.
- Crédito/Fatura.
- Bolsos/Liquidez.
- Mia approval flow.
- Metodologia.
- Configurações e privacidade.

5. Spreadsheet-aware patterns
Design how to show:
- source sheet/year/month/cell/range
- formula-preserving diff
- before/after values
- note changes
- checksum conflict
- rollback batch
- imported vs manual vs projected data
- reconciled lump vs bank detail
- shadowed/ignored import item
- source cell that is intentionally untouchable
- protected `Data`, `Saldo`, `Entradas`, `%`, and meaningful `Diário` notes
- phased ownership without confusing the user: in the current import-only phase the spreadsheet is canonical and SQLite is the local mirror; SQLite becomes system-of-record only in the future gated bidirectional phase (see `docs/adr/0003`)

6. Methodology-aware patterns
Design how to show:
- Régua 1 and Régua 2 side by side without merging their meanings.
- Diário as a single forward-planning number.
- Credit/fatura pressure without double-counting forecast impact.
- Reserve months and trend.
- Safe upfront vs installment comparison.
- Credit-to-debit migration progress.
- Debt payoff and stabilization plan.
- Mixed personal/business money separation.
- Household settlement and reimbursement flow.
- Post-reserve long-term allocation discipline.
- Recurring planned commitments.
- Reimbursement expected vs realized.
- Future manual projections vs app-generated projections.
- Ambiguous import conflicts that require review.
- Deliberate economy/savings vs leftover cash.
- Scenario copies/sandboxes before mutating the real plan.
- Current cash as transit layer vs reserve/restricted/illiquid wealth layers.
- Annual savings position (the 20–30% as a yearly average), realized vs optimistic-projected, without monthly pass/fail shaming.
- Predictability/coverage of each future month and the concrete gaps to fill, distinct from the realized picture.
- Cash ≠ performance: a growing buffer alongside a thin or negative monthly performance, both honest and visible.
- Adaptation gaps and the recognize-then-teach coaching for each (the cushion case as the worked example), with the named Map/Calibrate/Operate phase.
- A lump entry expanded into its noted per-person, per-item breakdown; pass-through gross vs net.

7. Mia rules
Define Mia’s UI behavior:
- no fake chat if feature is not implemented
- citations to deterministic tools and source cells
- no free-form financial math
- “I found / I suggest / approve to write” tone
- reviewable proposals only
- explicit approval and rejection states
- no writes without diff, validation, checksum, and approval

8. PT-BR copy system
Create copy rules and examples in PT-BR:
- money in BRL
- sentence case
- calm and direct
- no emoji
- no cat puns
- no moralizing
- clear privacy/control language
Include examples for empty, loading, error, warning, approval, conflict, rollback, and success states.

9. Accessibility contract
Define exact requirements for:
- WCAG 2.2 AA
- focus rings
- keyboard table navigation
- screen reader labels for money values
- status not conveyed by color only
- reduced motion
- target sizes
- dialogs/approval flows
- chart alternatives
- financial error prevention

10. Implementation handoff
Provide:
- recommended file/module structure
- token naming convention
- CSS custom property examples
- migration plan from current DS bundle/JSX to production TSX components
- first 10 implementation tasks in priority order
- acceptance checklist for design review

11. Repo reconciliation matrix
Provide a practical migration matrix with:
- current generated DS artifact or screen-local pattern
- proposed production TSX component/token target
- current/near-term/future status
- accessibility behavior to add
- tests or Storybook/specimen coverage needed
- deprecated English/USD/generic examples to replace

12. Motion system
Define the motion layer per the motion_and_wow section: duration and easing tokens, named transition presets (theme-reveal, screen-transition, row-stagger, value-resolve, lump-expand, coverage-fill), each with its `prefers-reduced-motion` fallback. State where motion is encouraged (transitions, check-in confirmation, list entrance, reveals) and where it is forbidden (animating a critical money value into existence, idle decorative loops on finance-critical surfaces). Extend the existing View Transitions circular-reveal language rather than inventing a competing one.

13. Dashboard information architecture
Before any card styling, deliver a Dashboard IA: the single hero, the secondary band, what collapses behind disclosure, what graduates to a dedicated screen, the reading order, and responsive behavior. The current screen is a degraded card stack; the redesign’s primary job is hierarchy and progressive disclosure, not more cards.</deliverables>

<quality_bar>
Before finalizing, self-check:

- No English/USD product examples.
- No fake personal data.
- No private names, source names, school names, course names, URLs, emails, or personal finance data.
- No generic SaaS dashboard visuals.
- No childish cat mascot behavior.
- No color-only statuses.
- No chart decoration near finance-critical data.
- No auto-approved AI writes.
- No component spec without accessibility behavior.
- Every money/table/diff pattern supports auditability and reconciliation.
- Every methodology reference is source-neutral and implementation-oriented.
- Every artifact labels current production vs near-term implementation vs future reference.
- No design asks the app to overwrite protected spreadsheet structure or meaningful manual notes.
- The Dashboard is delivered as an information architecture with a clear hero and disclosure, not as another vertical stack of equal-weight cards.
- Cell notes are treated as real ledger detail, not discarded; no entry is reduced to a generic “Saída <month>” label in any mock.
- `Economia = 0` / `Diário = 0` are shown as adaptation/credit-first states, never as “measured zero”.
- Savings is judged on the annual position; no single month is shamed as “below target”.
- Every motion has a reduced-motion fallback, and no animation delays the reading of a critical money value; motion never crosses into gamified bounce/confetti/streak territory.
- Coaching copy recognizes before it teaches; no “you should / you failed / you missed” voice.</quality_bar>

<output_format>
Use clear headings and tables where useful.
Be concrete: token names, component variants, states, measurements, behavior.
Do not write production React code unless needed for a small illustrative contract.
End with a prioritized implementation checklist.
</output_format>
```

## Suggested Interaction Strategy

Use this as a single-run prompt when Claude Design needs to produce an updated implementation-ready design-system spec. If you can afford two runs, use the first run only to critique the existing `Midnight Ledger` system against the product/specs, then ask Claude Design to produce the final revised system from that critique.

If methodology material needs to influence the design, first distill it locally into source-neutral rules and edge cases. Never attach raw private source files to the design session.
