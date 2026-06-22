/* Neko Finance — Tela Mia / Copiloto (stub Em desenvolvimento).
   Mostra o header da Mia com badge de aviso, texto explicativo e a seção
   "O que a Mia já sabe" com fatos determinísticos do método.
   Expõe window.CopilotScreen. */
const NS = window.NekoFinanceDesignSystem_9bd1cd;
const { Badge, MiaAvatar } = NS;
const Icon = window.Icon;

const copCSS = `
.cop{display:flex;flex-direction:column;gap:var(--space-6);max-width:680px;}

/* Painel principal da Mia */
.cop-panel{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-lg);
  box-shadow:var(--shadow-1);padding:var(--space-7);}

/* Cabeçalho: avatar + nome + badge */
.cop-header{display:flex;align-items:center;gap:var(--space-5);margin-bottom:var(--space-5);}
.cop-header__meta{flex:1;min-width:0;}
.cop-header__label{font-size:var(--fs-micro);font-weight:var(--fw-bold);letter-spacing:var(--ls-caps);
  text-transform:uppercase;color:var(--text-faint);line-height:1;margin-bottom:3px;}
.cop-header__name{font-size:var(--fs-h3);font-weight:var(--fw-bold);color:var(--text-strong);
  letter-spacing:var(--ls-snug);line-height:1.1;}

/* Texto explicativo */
.cop-desc{font-size:var(--fs-body);line-height:1.6;color:var(--text-muted);margin:0;}

/* Seção "O que a Mia já sabe" */
.cop-facts{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);
  box-shadow:var(--shadow-1);padding:var(--space-6);}
.cop-facts__head{font-size:var(--fs-label);font-weight:var(--fw-semibold);letter-spacing:var(--ls-label);
  text-transform:uppercase;color:var(--text-muted);margin:0 0 var(--space-5);}
.cop-facts__list{list-style:none;margin:0;padding:0;display:flex;flex-direction:column;gap:var(--space-4);}
.cop-fact{display:flex;gap:var(--space-3);align-items:baseline;font-size:var(--fs-body);color:var(--text);
  line-height:1.5;}
.cop-fact__arrow{color:var(--primary);flex:none;font-family:var(--font-mono);font-size:var(--fs-sm);}
.cop-fact__money{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-weight:var(--fw-semibold);
  color:var(--text-strong);}

/* Seção roadmap "O que a Mia vai fazer" */
.cop-road{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);
  box-shadow:var(--shadow-1);padding:var(--space-6);}
.cop-road__head{font-size:var(--fs-label);font-weight:var(--fw-semibold);letter-spacing:var(--ls-label);
  text-transform:uppercase;color:var(--text-muted);margin:0 0 var(--space-5);}
.cop-road__list{list-style:none;margin:0;padding:0;display:flex;flex-direction:column;gap:var(--space-5);
  counter-reset:road;}
.cop-roaditem{display:flex;gap:var(--space-4);font-size:var(--fs-body);color:var(--text);line-height:1.5;
  counter-increment:road;}
.cop-roaditem__num{flex:none;width:22px;height:22px;border-radius:50%;background:var(--primary-quiet);
  color:var(--primary-quiet-text);font-size:var(--fs-micro);font-weight:var(--fw-bold);
  display:flex;align-items:center;justify-content:center;margin-top:1px;}
`;

function injectCopCSS() {
  if (document.getElementById("copilot-css")) return;
  const s = document.createElement("style");
  s.id = "copilot-css";
  s.textContent = copCSS;
  document.head.appendChild(s);
}

/* Fatos determinísticos representativos (valores fixos para o ui_kit) */
const FACTS = [
  <>
    Sua reserva cobre <span className="cop-fact__money">7,3</span> meses de custo de
    vida (a meta mínima é 6).
  </>,
  <>
    No ano, você economizou <span className="cop-fact__money">24%</span> (referência
    20–30%).
  </>,
  <>
    Você pode gastar até <span className="cop-fact__money">R$ 312,40</span> hoje sem
    furar suas metas.
  </>,
];

const ROADMAP = [
  "Diagnóstico em linguagem natural: padrões de gasto, evolução da reserva e o peso real do crédito — sempre em modo leitura.",
  "Respostas a decisões: “posso comprar?”, “à vista ou parcelado?” — usando o saldo projetado, nunca cálculo improvisado.",
  "Escrita na planilha somente com a sua aprovação explícita, mostrando um diff antes → depois de cada alteração.",
];

function CopilotScreen() {
  injectCopCSS();
  return (
    <div className="cop">
      {/* Painel de identidade da Mia */}
      <div className="cop-panel">
        <div className="cop-header">
          <MiaAvatar width={48} height={48} />
          <div className="cop-header__meta">
            <div className="cop-header__label">Copiloto</div>
            <div className="cop-header__name">Mia</div>
          </div>
          <Badge tone="warning">Em desenvolvimento</Badge>
        </div>
        <p className="cop-desc">
          O chat da Mia ainda não está disponível nesta versão. Tudo o que você vê no
          app hoje é calculado pelo motor determinístico — nada é gerado por IA.
        </p>
      </div>

      {/* O que a Mia já sabe */}
      <section aria-labelledby="cop-knows-title" className="cop-facts">
        <h2 id="cop-knows-title" className="cop-facts__head">
          O que a Mia já sabe · números do método, sem IA
        </h2>
        <ul className="cop-facts__list">
          {FACTS.map((fact, i) => (
            <li key={i} className="cop-fact">
              <span className="cop-fact__arrow" aria-hidden="true">
                ↳
              </span>
              <span>{fact}</span>
            </li>
          ))}
        </ul>
      </section>

      {/* Roadmap */}
      <section aria-labelledby="cop-road-title" className="cop-road">
        <h2 id="cop-road-title" className="cop-road__head">
          O que a Mia vai fazer
        </h2>
        <ol className="cop-road__list">
          {ROADMAP.map((item, i) => (
            <li key={i} className="cop-roaditem">
              <span className="cop-roaditem__num" aria-hidden="true">
                {i + 1}
              </span>
              <span>{item}</span>
            </li>
          ))}
        </ol>
      </section>
    </div>
  );
}
window.CopilotScreen = CopilotScreen;
