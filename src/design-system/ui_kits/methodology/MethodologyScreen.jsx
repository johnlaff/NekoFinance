/* Neko Finance — Ajuda / princípios do método. Lista de 7 cards de princípio
   com hero intro. Expõe window.MethodologyScreen. */
const NS = window.NekoFinanceDesignSystem_9bd1cd;
const { Badge } = NS;
const Icon = window.Icon;

const metCSS = `
.met{display:flex;flex-direction:column;gap:20px;max-width:1080px;}
.met-hero{padding:18px 20px;background:var(--surface);border:1px solid var(--border);
  border-radius:var(--radius-lg);box-shadow:var(--shadow-1);}
.met-hero__eyebrow{font-size:11px;font-weight:700;letter-spacing:.08em;text-transform:uppercase;
  color:var(--primary);margin-bottom:8px;}
.met-hero__line{font-size:15px;line-height:1.55;color:var(--text-muted);}
.met-hero__line b{color:var(--text-strong);font-weight:700;}
.met-grid{display:grid;grid-template-columns:repeat(3,1fr);gap:14px;}
.met-card{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);
  box-shadow:var(--shadow-1);padding:18px 18px 20px;display:flex;flex-direction:column;gap:10px;}
.met-card__ic{width:34px;height:34px;border-radius:var(--radius-sm);
  background:var(--primary-quiet);color:var(--primary);
  display:flex;align-items:center;justify-content:center;flex:none;}
.met-card__title{font-size:14px;font-weight:700;color:var(--text-strong);line-height:1.3;
  letter-spacing:-0.01em;}
.met-card__body{font-size:13.5px;line-height:1.6;color:var(--text-muted);flex:1;}
.met-card__body b{color:var(--text);font-weight:600;}
.met-card__body em{font-style:italic;color:var(--text);}
@media (max-width:960px){.met-grid{grid-template-columns:repeat(2,1fr);}}
@media (max-width:600px){.met-grid{grid-template-columns:1fr;}}
@media (prefers-reduced-motion:reduce){*{transition:none!important;animation:none!important;}}
`;

function injectMet() {
  if (document.getElementById("met-css")) return;
  const s = document.createElement("style");
  s.id = "met-css";
  s.textContent = metCSS;
  document.head.appendChild(s);
}

const PRINCIPLES = [
  {
    icon: "trendingUp",
    title: "Saldo projetado, não saldo atual",
    body: (
      <>
        A pergunta que importa não é <em>"quanto eu tenho?"</em>, e sim{" "}
        <em>"quanto vai sobrar?"</em>. O Neko encadeia dia a dia as entradas e saídas
        futuras e mostra o <b>saldo projetado</b> para o fim do mês: esse é o número
        herói do dashboard.
      </>
    ),
  },
  {
    icon: "sliders",
    title: "A conta do mês (Performance)",
    body: (
      <>
        Performance = Entradas − (Saídas + Diário + Economia + previsão do diário que
        ainda falta). As Saídas já incluem as contas fixas e a <b>fatura do cartão</b> —
        que entra como saída no vencimento, sem coluna própria. Por isso o mês nasce no
        vermelho e vai esverdeando conforme o diário real fica abaixo do teto.
      </>
    ),
  },
  {
    icon: "calendarRange",
    title: "Custo de vida",
    body: (
      <>
        Custo de vida = <b>Saídas</b> (contas fixas previsíveis + fatura do cartão no
        vencimento) + <b>Diário</b> (o resto). O diário é um número único por dia, não
        um orçamento por categoria: categorias servem para diagnóstico,{" "}
        <em>nunca para planejamento</em>.
      </>
    ),
  },
  {
    icon: "piggy",
    title: "Guardar 20 a 30%",
    body: (
      <>
        Economizado = o quanto você transfere para a reserva ÷ entradas. A meta é{" "}
        <b>20 a 30%</b> — mas como <em>média do ano</em>, não de cada mês (uns meses
        mais, outros menos). É diferente do colchão: a Economia é o que você separa de
        propósito.
      </>
    ),
  },
  {
    icon: "creditCard",
    title: "Débito e crédito: dois ritmos",
    body: (
      <>
        Débito, PIX e dinheiro afetam o caixa no mesmo dia. O crédito é diferente: cada
        compra vai para a fatura e o Neko lança esse total como uma{" "}
        <b>Saída única no vencimento</b> — o cartão sequestra o salário futuro. Por isso
        a fatura aparece nas Saídas, não no Diário.
      </>
    ),
  },
  {
    icon: "shield",
    title: "Reserva em meses",
    body: (
      <>
        A reserva de emergência é medida em <b>meses de custo de vida</b> (reserva ÷
        custo mensal), não em valor absoluto. A meta mínima é <b>6 meses</b>; a partir
        de 12 é a paz financeira, e o excedente pode trabalhar em outro lugar.
      </>
    ),
  },
  {
    icon: "calculator",
    title: "Cálculo determinístico",
    body: (
      <>
        Todos os números vêm de um <b>motor de cálculo determinístico</b> e testado. A
        Mia (em desenvolvimento) vai explicar e contextualizar esses números sem nunca
        inventar contas; e nenhuma escrita na sua planilha acontece sem a sua{" "}
        <em>aprovação explícita</em>.
      </>
    ),
  },
];

function MethodologyScreen() {
  injectMet();
  return (
    <div className="met">
      <div className="met-hero">
        <div className="met-hero__eyebrow">Ajuda</div>
        <div className="met-hero__line">
          <b>Previsibilidade primeiro.</b> O Neko organiza suas finanças em torno de uma
          única disciplina: saber hoje como o mês termina. Os sete princípios abaixo
          explicam como cada número é calculado.
        </div>
      </div>

      <div className="met-grid">
        {PRINCIPLES.map((p) => (
          <article className="met-card" key={p.title}>
            <span className="met-card__ic">
              <Icon name={p.icon} size={18} stroke={1.75} />
            </span>
            <h2 className="met-card__title">{p.title}</h2>
            <p className="met-card__body">{p.body}</p>
          </article>
        ))}
      </div>
    </div>
  );
}
window.MethodologyScreen = MethodologyScreen;
