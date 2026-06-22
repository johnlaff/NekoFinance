Mia's brand avatar — an inline SVG cat-ear silhouette with jade fill on a dark rounded tile, used as the identity mark for Neko Finance's AI copilot wherever her name appears.

```jsx
<MiaAvatar />
<MiaAvatar width={48} height={48} />
<MiaAvatar width={24} height={24} style={{ borderRadius: "var(--radius-sm)" }} />
```

`width` and `height` default to 40 × 40. The background rect uses `var(--surface-elevated)` and the icon fill uses `var(--primary)`, so both adapt to light and dark themes via CSS custom properties with hard-coded fallbacks (`#1F2827` / `#3FBF8F`). The SVG carries `role="img"` and `aria-label="Mia, copiloto financeiro"` for accessibility. Pass `className` or `style` to adjust position or sizing context in the parent layout.
