import { CalendarRange, Ruler, ShieldCheck, Sigma, TrendingUp } from "lucide-react";

const PRINCIPLES = [
  {
    icon: TrendingUp,
    title: "Saldo projetado, não saldo atual",
    body: "A pergunta que importa não é “quanto eu tenho?”, e sim “quanto vai sobrar?”. O Neko encadeia dia a dia as entradas e saídas futuras e mostra o saldo projetado para o fim do mês — esse é o número herói do dashboard.",
  },
  {
    icon: CalendarRange,
    title: "Saídas fixas e diário",
    body: "Custo de vida = saídas fixas (contas previsíveis, com data) + diário (o resto). O diário é um número único por dia, não um orçamento por categoria: categorias servem para diagnóstico, nunca para planejamento.",
  },
  {
    icon: Ruler,
    title: "Régua 1 e Régua 2",
    body: "Débito, PIX e dinheiro afetam o caixa no dia (Régua 1). Crédito acumula na fatura e só pesa no vencimento (Régua 2). O Neko acompanha as duas réguas de forma independente — isso evita o autoengano de um diário “zerado” enquanto a fatura cresce em silêncio.",
  },
  {
    icon: ShieldCheck,
    title: "Reserva em meses",
    body: "A reserva de emergência é medida em meses de custo de vida (reserva ÷ custo mensal), não em valor absoluto. A meta inicial é 6 meses; acima de 12, o excedente pode trabalhar em outro lugar.",
  },
  {
    icon: Sigma,
    title: "Cálculo determinístico",
    body: "Todos os números vêm de um motor de cálculo determinístico e testado. A Mia explica e contextualiza — ela nunca inventa contas, e nenhuma escrita na sua planilha acontece sem a sua aprovação explícita.",
  },
];

export function MethodologyScreen() {
  return (
    <div className="dash">
      <div className="dash-hero">
        <div className="dash-hero__txt">
          <div className="dash-hero__line">
            <b>Previsibilidade primeiro.</b> O Neko organiza suas finanças em torno de
            uma única disciplina: saber hoje como o mês termina.
          </div>
        </div>
      </div>

      <div className="met-grid">
        {PRINCIPLES.map((p) => (
          <article className="dash-card met-card" key={p.title}>
            <span className="met-card__ic">
              <p.icon size={18} strokeWidth={1.75} />
            </span>
            <h2 className="met-card__title">{p.title}</h2>
            <p className="met-card__body">{p.body}</p>
          </article>
        ))}
      </div>
    </div>
  );
}
