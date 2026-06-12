# Claude Design Prompt

Use this prompt in Claude Design or a design-focused Claude session. It assumes the repo already has an initial design system and asks Claude Design to refine it into a production-ready, domain-specific system.

Updated 2026-06-12 with repo/spec analysis, the public example workbook structure, source-neutral methodology synthesis, and 2026 market/state-of-the-art design-system research.

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
- SQLite is the system of record; the spreadsheet remains the canonical view of the method.
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
- SafeToSpendCallout
- DeficitNotice
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
- Dashboard forecast view.
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
- “planilha canônica, SQLite system-of-record” without confusing the user

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
</deliverables>

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
</quality_bar>

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
