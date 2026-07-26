// As telas vivem fora do arquivo de componente porque são dados de navegação — o manifesto de paridade da conversa as enumera.

export type Screen =
  | "hoje"
  | "lancamentos"
  | "mes"
  | "cartoes"
  | "ano"
  | "calendario"
  | "horizonte"
  | "tags"
  | "mia"
  | "teto"
  | "config";

export const SCREEN_META: Record<Screen, { title: string; crumb: string }> = {
  hoje: { title: "Hoje", crumb: "Quanto posso gastar hoje" },
  lancamentos: { title: "Lançamentos", crumb: "Seu livro-razão" },
  mes: { title: "Este mês", crumb: "Como o mês está indo" },
  cartoes: { title: "Cartões", crumb: "Faturas, séries e reembolsos" },
  ano: { title: "O ano", crumb: "O ano num olhar" },
  calendario: { title: "Calendário", crumb: "Saúde do saldo dia a dia" },
  horizonte: { title: "Horizonte", crumb: "Para onde o saldo vai" },
  tags: { title: "Tags", crumb: "O que as réguas enxergam" },
  mia: { title: "Mia", crumb: "Sua copilota financeira" },
  // Fora da nav (destino de CTA a partir da Hoje e de Configurações), mas com meta própria.
  teto: { title: "Teto do diário", crumb: "A cerimônia do gasto variável" },
  config: { title: "Configurações", crumb: "Conexão e privacidade" },
};

/** As telas do app em tempo de execução — o manifesto de paridade da conversa as enumera. */
export const SCREEN_KEYS = Object.keys(SCREEN_META) as Screen[];
