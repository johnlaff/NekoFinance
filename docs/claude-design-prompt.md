# Claude Design Prompt

Use this prompt in Claude Design or a design-focused Claude session. It is written to minimize follow-up usage by giving full context, concrete constraints, and explicit deliverables.

```text
You are a senior product designer and design systems lead for a local-first fintech desktop app.

<context>
Product name: Neko Finance.
Product type: Tauri desktop app with React UI, later also responsive web-like layouts for smaller screens.
Primary user: one person managing personal finances from Google Sheets, with a likely future second user/partner.
Core value: local-first private finance dashboard plus AI copilot that reads Google Sheets, explains finances, separates ownership/responsibility, and proposes safe sheet changes only after human approval.
Personality: credible fintech first, subtle cat/neko warmth second. It should not feel childish, crypto, anime-heavy, or like a generic neobank clone.
Copilot persona: Mia, a calm financial copilot. Mia can have subtle feline cues in language/visuals, but the app must remain serious and trustworthy.
Important domain nuance: expenses must distinguish personal spending, partner/additional-card spending, shared expenses, payer, beneficiary, and responsible owner.
Privacy stance: local-first, private by default, no backend in MVP, human-approved writes.
Current stack: Tauri 2, React 19, TypeScript, SQLite planned, Google Sheets API planned, AI agent tools planned.
</context>

<product_surfaces>
Design for these MVP screens:
1. Main dashboard with financial health status, cashflow summary, category breakdown, account/card overview, and owner/responsibility separation.
2. Transactions/import review screen with Google Sheets mapping, category/owner assignment, and confidence states.
3. Copilot screen with chat, cited calculations, deterministic tool outputs, and proposed Google Sheets diff requiring approval.
4. Methodology/insights screen showing private source-neutral rules, not public course/source references.
5. Settings/privacy screen for local data, OAuth connection, AI provider keys, and update channel.
</product_surfaces>

<design_goals>
Create a design system proposal that feels distinctive, premium, calm, and useful for dense financial data.
Avoid generic AI dashboard aesthetics: no purple gradient SaaS look, no default cream/terracotta Claude house style, no bland Inter-only enterprise UI, no childish cat illustrations, no crypto/NFT visual language.
Make the app feel like a private financial cockpit: focused, tactile, trustworthy, with a small amount of feline warmth.
Support both light and dark themes, but choose one primary theme to present first.
Prioritize legibility for money values, tables, charts, status colors, and approval diffs.
Design for desktop first, with responsive behavior for a narrow mobile-like width.
</design_goals>

<deliverables>
Do not ask clarifying questions unless a blocker is truly unavoidable. Make reasonable assumptions and state them briefly.

First, propose 4 distinct visual directions. For each direction include: name, one-line rationale, background hex, surface hex, primary accent hex, risk/status colors, typography pairing, and where subtle cat/neko cues appear.

Then choose the strongest direction for this product and fully develop it into a design system.

For the chosen direction, provide:
1. Brand principles and design adjectives.
2. Color tokens with hex values for background, surface, elevated surface, border, text, muted text, primary, secondary, success, warning, danger, info, and chart series.
3. Typography system with recommended freely available fonts or system fallbacks, sizes, weights, line heights, and usage rules for money, headings, tables, chat, labels, and annotations.
4. Spacing, radius, elevation, border, and motion tokens.
5. Component specs for buttons, inputs, segmented controls, cards, metric tiles, charts, transaction rows, owner chips, health badges, approval diff cards, chat bubbles, tool-result citations, empty states, loading states, and error states.
6. Chart language: colors, line/bar/donut style, gridlines, labels, thresholds, and annotation behavior.
7. Data-density rules for desktop dashboards and transaction tables.
8. Accessibility constraints: contrast, focus rings, keyboard states, status color plus label requirements, reduced motion behavior.
9. Three concrete screen layouts described in enough detail that a frontend engineer can implement them: dashboard, transactions/import review, copilot approval flow.
10. A short implementation handoff with CSS custom property names and a recommended component inventory.
</deliverables>

<output_format>
Use clear headings and tables where useful.
Be concrete. Prefer exact tokens, measurements, and component behavior over moodboard language.
Do not produce production React code in this response. This session is for design system direction and implementation-ready specs.
End with a concise checklist of what the engineer should implement first.
</output_format>

<quality_bar>
Before finalizing, self-check that the proposal is not generic, not childish, not overdecorated, and not visually confused with Pierre or any existing finance app.
Make sure the design supports finance accuracy, privacy, multi-person ownership, and human approval as first-class UI concepts.
</quality_bar>
```

## Suggested Interaction Strategy

If you only have one Claude Design run, use the full prompt above. If you can afford two runs, use the first run only through the 4 visual directions, pick one manually, then ask Claude to fully develop only the chosen direction. The two-run approach usually produces stronger design variety and avoids the model defaulting to a single house style too early.
