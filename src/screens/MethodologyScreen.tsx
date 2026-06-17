import {
  CalendarRange,
  Gauge,
  PiggyBank,
  Ruler,
  ShieldCheck,
  Sigma,
  TrendingUp,
} from "lucide-react";

const PRINCIPLES = [
  {
    icon: TrendingUp,
    title: "Saldo projetado, não saldo atual",
    body: "A pergunta que importa não é “quanto eu tenho?”, e sim “quanto vai sobrar?”. O Neko encadeia dia a dia as entradas e saídas futuras e mostra o saldo projetado para o fim do mês: esse é o número herói do dashboard.",
  },
  {
    icon: Gauge,
    title: "A conta do mês (Performance)",
    body: "Performance = Entradas − (Saídas + Diário + Economia + previsão do diário que ainda falta). As Saídas já incluem as contas fixas e a fatura do cartão — que entra como saída no vencimento, sem coluna própria (não é dupla contagem). Por isso o mês nasce no vermelho e vai esverdeando conforme o diário real fica abaixo do teto.",
  },
  {
    icon: CalendarRange,
    title: "Custo de vida",
    body: "Custo de vida = Saídas (contas fixas previsíveis, com data, + a fatura do cartão no vencimento) + Diário (o resto). O diário é um número único por dia, não um orçamento por categoria: categorias servem para diagnóstico, nunca para planejamento.",
  },
  {
    icon: PiggyBank,
    title: "Guardar 20 a 30%",
    body: "Economizado = o quanto você transfere para a reserva ÷ entradas. A meta é 20 a 30% — mas como MÉDIA do ano, não de cada mês (uns meses mais, outros menos). É diferente do colchão (o que sobra em caixa sem você transferir): a Economia é o que você separa de propósito.",
  },
  {
    icon: Ruler,
    title: "Débito e crédito: dois ritmos",
    body: "Débito, PIX e dinheiro afetam o caixa no mesmo dia. O crédito acumula na fatura e só pesa no vencimento. O Neko acompanha os dois de forma independente: isso evita o autoengano de um diário “zerado” enquanto a fatura cresce em silêncio.",
  },
  {
    icon: ShieldCheck,
    title: "Reserva em meses",
    body: "A reserva de emergência é medida em meses de custo de vida (reserva ÷ custo mensal), não em valor absoluto. A meta mínima é 6 meses; a partir de 12 é a paz financeira, e o excedente pode trabalhar em outro lugar.",
  },
  {
    icon: Sigma,
    title: "Cálculo determinístico",
    body: "Todos os números vêm de um motor de cálculo determinístico e testado. A Mia (em desenvolvimento) vai explicar e contextualizar esses números sem nunca inventar contas; e nenhuma escrita na sua planilha acontece sem a sua aprovação explícita.",
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
