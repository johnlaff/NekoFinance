Method-adaptation phase badge — shows where the user is on the Mapear → Calibrar → Operar journey with a 3-segment progress indicator. Calm, non-punitive: it marks progress without demanding maturity the user hasn't reached.

```jsx
<PhaseBadge phase="map" />
<PhaseBadge phase="calibrate" />
<PhaseBadge phase="operate" />
```

`phase` fills 1, 2 or 3 segments in jade (`--primary`); remaining segments stay muted (`--surface-2`). The visible label and dots are `aria-hidden`; a screen-reader-only string announces the phase and position ("Fase de adaptação: Calibrar (2 de 3)"). Use it inline next to a section title on the Methodology screen or the dashboard header.
