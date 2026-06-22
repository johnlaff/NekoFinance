Badge de tipo de movimento com círculo colorido e glifo de letra para cada um dos 5 pilares do método (Entrada, Saída, Diário, Economia, Cartão).

```jsx
// Apenas o glifo (padrão) — rótulo disponível só para leitores de tela
<MovBadge kind="entrada" />
<MovBadge kind="saida" />
<MovBadge kind="diario" />
<MovBadge kind="economia" />
<MovBadge kind="cartao" />

// Com rótulo visível ao lado do glifo
<MovBadge kind="entrada"  showLabel size={20} />
<MovBadge kind="saida"    showLabel size={20} />
<MovBadge kind="diario"   showLabel size={20} />
<MovBadge kind="economia" showLabel size={20} />
<MovBadge kind="cartao"   showLabel size={20} />

// Glifo maior (e.g. em cabeçalhos de seção)
<MovBadge kind="saida" size={28} showLabel />
```

`kind` seleciona o pilar — as cores vêm dos tokens `--type-entrada`, `--type-saida`, `--type-diario`, `--type-economia`, `--type-cartao`. Entrada e Economia compartilham o glifo "E"; a cor os distingue (jade vs. verde-escuro). `showLabel` torna o nome visível; sem ele o nome ainda é exposto a leitores de tela via span sr-only (acessível sem ser color-only). `size` controla o diâmetro do círculo em px (fonte calculada automaticamente como 56% do diâmetro). O rótulo visível usa `--fs-sm` e `--text`.
