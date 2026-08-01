# ADR-0005: One Annual Ruler, with Patrimônio Outside It

The method's Economizado% — Economia ÷ income, judged against a 20–30% band as an **annual**
average — is the figure the product exists to keep honest. It is read by the Ano screen, by the
Totais screen, by the conversation, and by the card-mode gate. Deriving it more than once is how
one app ends up showing a year two different ways.

## Decision

**`forecast::annual_ruler` is the only definition of the annual ruler.** Every surface reads it;
no DTO, facade, gate or screen recomputes it. A caller that needs the figure calls the function —
copying the arithmetic is the failure mode this decision closes.

Three properties are part of the definition, not of the callers:

- **Window: the months lived, the current month included.** A month in course counts as soon as it
  has movement. The ruler never waits for a month to close.
- **Truncation.** The percentage truncates, like the monthly engine and like every percentage the
  app prints. A figure rounded up would promise what the engine did not measure.
- **Numerator: Economia launched.** Transfers to reserve, and nothing else.

**Patrimônio — pension, FGTS, anything illiquid — never enters the ruler.** Not at any reserve
coverage, not under any condition. It is a bucket of its own, published beside the ruler so it can
be read, never summed into it. This is classification, not advice: the money is still the person's,
it simply is not the accessible Economia the band measures.

The savings guardrail is the single deliberate exception on window: it reads
`registered_economia_cents` over **complete** months. It decides how much can be spent today, and a
denominator still forming mid-month would surface as false alarm. It is a decision input, not a
published figure.

## Why

The three sources the product tracks agree, and the code did not:

- The spreadsheet computes the year as `TOTAL Economia ÷ TOTAL income` across the twelve month
  rows, with no complete-month gate — the running month is in the sum.
- The method teaches Economizado% as launched Economia over income, with the band as an annual
  average, and treats pension and FGTS as patrimônio built **after** the reserve — a sequence for
  the person, never a term in the ruler.
- The reference app computes the same ratio over its five movement types, with no conditional
  branch anywhere.

Against that, the code carried three implementations: a DTO on complete months rounding half-up, the
engine on lived months truncating, and a third reading inside the card gate. The DTO also applied a
rule with no source behind it — pension counting as Economia once the reserve covered six months —
which inflated the ruler and could make the card gate call an economy alive that the year screen was
already showing below the band.

## Consequences

- The conversation, the Ano screen, the Totais screen and the card gate print one number for one
  year. The receipt closes: its operands produce the percentage it prints.
- The card gate got stricter wherever the invented rule had been padding the numerator.
- **The product classifies; it does not counsel.** Copy states that patrimônio sits outside the
  ruler and stops there — it never prescribes an order between saving and investing, and never reads
  a contribution as a mistake. Employer-matched pension is the plain case: leaving it would cost the
  person money, and an app that nudged against it would be wrong about their life, whatever the
  ruler says.
- Regression tests hold the line at both seams: pension stays out of the ruler at any reserve
  coverage, and the conversation and the screen are asserted to read the same ruler.
