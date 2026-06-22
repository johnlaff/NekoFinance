App logo mark — the Neko cat-face SVG glyph, scalable to any size and tintable via the `color` prop or CSS `currentColor`.

```jsx
<NekoMark />
<NekoMark width={32} height={32} color="var(--text-faint)" aria-hidden={true} />
<NekoMark width={64} height={64} color="var(--primary)" aria-label="Neko Finance" />
```

`width` and `height` control rendered size (default 48×48). `color` sets the fill — any CSS color or design-token `var()` is accepted; the default is `var(--primary)` (jade). Pass `aria-hidden={true}` when the mark is decorative and sits inside an already-labelled control. Tokens used: `--primary`.
