import { useEffect, useRef, useState, type CSSProperties } from "react";
import { ArrowLeft, ArrowRight, Check, Sparkles } from "lucide-react";
import { Button } from "../../design-system/components/Button";
import { MovBadge, type MovKind } from "../../design-system/components/MovBadge";
import { setAppSetting } from "../../lib/api";
import { NewTransactionForm } from "../../screens/NewTransactionForm";

export const ONBOARDING_KEY = "onboarding_done";

const TYPES: { kind: MovKind; desc: string }[] = [
  { kind: "entrada", desc: "o que entra (salário, renda)" },
  { kind: "saida", desc: "saídas fixas (aluguel, contas)" },
  { kind: "diario", desc: "o gasto variável do dia a dia" },
  { kind: "cartao", desc: "compras no cartão (vira fatura)" },
  { kind: "economia", desc: "guardar: sua poupança do mês" },
];

const TOTAL_STEPS = 5;

// `<dialog open>` nativo (não-modal): a modalidade — scrim, foco e trap — é gerida no componente
// (showModal() tem suporte irregular no WebView/jsdom). Estes estilos resetam os defaults do dialog
// (borda/margin/limites) e desenham o scrim em cima do app.
const OVERLAY_STYLE: CSSProperties = {
  position: "fixed",
  inset: 0,
  zIndex: 50,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  padding: "var(--space-5)",
  margin: 0,
  border: "none",
  maxWidth: "none",
  maxHeight: "none",
  width: "100%",
  height: "100%",
  background: "var(--bg-scrim)",
  backdropFilter: "blur(8px)",
};

const CARD_STYLE: CSSProperties = {
  width: "100%",
  maxWidth: 520,
  maxHeight: "92vh",
  overflowY: "auto",
  outline: "none",
  background: "var(--surface-elevated)",
  border: "var(--bw-hair) solid var(--border-strong)",
  borderRadius: "var(--radius-xl)",
  boxShadow: "var(--shadow-4)",
  padding: "var(--space-7)",
};

const pStyle: CSSProperties = {
  color: "var(--text)",
  fontSize: "var(--fs-body)",
  lineHeight: 1.55,
  margin: "var(--space-3) 0 0",
};

function OnboardingHeader({ step }: { step: number }) {
  return (
    <>
      {/* Marca + passo */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: "var(--space-4)",
        }}
      >
        <span
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: "var(--space-2)",
            fontWeight: "var(--fw-bold)",
            fontSize: "var(--fs-body)",
            color: "var(--text-strong)",
            letterSpacing: "var(--ls-tight)",
          }}
        >
          <span
            aria-hidden="true"
            style={{
              width: 9,
              height: 9,
              borderRadius: "50%",
              background: "var(--primary)",
              boxShadow: "0 0 0 4px var(--primary-quiet)",
            }}
          />
          Neko
        </span>
        <span
          style={{
            fontSize: "var(--fs-label)",
            fontWeight: "var(--fw-semibold)",
            letterSpacing: "var(--ls-label)",
            textTransform: "uppercase",
            color: "var(--text-faint)",
          }}
        >
          {step + 1} / {TOTAL_STEPS}
        </span>
      </div>

      {/* Progresso */}
      <div style={{ display: "flex", gap: 6, marginBottom: "var(--space-5)" }}>
        {Array.from({ length: TOTAL_STEPS }, (_, i) => (
          <span
            key={i}
            aria-hidden="true"
            style={{
              flex: 1,
              height: 4,
              borderRadius: "var(--radius-pill)",
              backgroundColor: i <= step ? "var(--primary)" : "var(--bg-subtle)",
              // `--t-hover` é uma LISTA de transições (background/border/color); prefixar uma
              // propriedade ("background var(--t-hover)") é sintaxe inválida. Declaramos explícito.
              transition: "background-color var(--dur-fast) var(--ease-standard)",
            }}
          />
        ))}
      </div>
    </>
  );
}

function StepContent({
  step,
  next,
  finish,
  onGoToSettings,
}: {
  step: number;
  next: () => void;
  finish: () => void;
  onGoToSettings?: (() => void) | undefined;
}) {
  if (step === 0) {
    return (
      <Step
        title="Bem-vindo ao Neko"
        subtitle="Seu dinheiro, previsível. Não mais uma planilha que você esquece."
      >
        <p style={pStyle}>
          O Neko fala a língua do método: cinco tipos de movimento, não dezenas de
          categorias. É assim que você lê suas finanças:
        </p>
        <div
          style={{
            display: "grid",
            gap: "var(--space-3)",
            marginTop: "var(--space-3)",
          }}
        >
          {TYPES.map((t) => (
            <div
              key={t.kind}
              style={{ display: "flex", alignItems: "center", gap: "var(--space-3)" }}
            >
              <MovBadge kind={t.kind} showLabel size={18} />
              <span style={{ color: "var(--text-muted)", fontSize: "var(--fs-sm)" }}>
                — {t.desc}
              </span>
            </div>
          ))}
        </div>
      </Step>
    );
  }

  if (step === 1) {
    return (
      <Step
        title="Previsível > categorizar"
        subtitle="A pergunta não é “onde gastei?”, é “sobra ou falta até o fim do mês?”."
      >
        <p style={pStyle}>
          Apps comuns te fazem etiquetar cada compra e olhar para trás. O método olha
          para a <b>frente</b>: o que já está lançado e o que ainda falta, para você
          saber hoje quanto pode gastar sem furar o mês.
        </p>
        <p style={pStyle}>
          O Neko calcula isso de forma determinística, sem achismo e sem IA inventando
          conta. Você decide; o app prevê.
        </p>
      </Step>
    );
  }

  if (step === 2) {
    return (
      <Step
        title="Traga seus dados"
        subtitle="Comece da sua planilha ou do zero: você escolhe."
      >
        <p style={pStyle}>
          Você pode <b>conectar o Google Sheets</b>, <b>importar um .xlsx</b> local, ou{" "}
          <b>começar do zero</b> lançando aqui mesmo. Nada sai do seu computador sem
          você pedir.
        </p>
        {onGoToSettings && (
          <Button
            variant="secondary"
            onClick={() => {
              finish();
              onGoToSettings();
            }}
          >
            Ir para Configurações e importar
          </Button>
        )}
      </Step>
    );
  }

  if (step === 3) {
    return (
      <Step
        title="Seu primeiro lançamento"
        subtitle="Registre algo de hoje: um café, o salário, o aluguel."
      >
        <p style={pStyle}>
          Experimente agora. Escolha o tipo e o valor. É assim que o dia a dia entra no
          Neko.
        </p>
        <div style={{ marginTop: "var(--space-3)" }}>
          <NewTransactionForm onCreated={next} />
        </div>
      </Step>
    );
  }

  return (
    <Step
      title="Sua meta de poupança"
      subtitle="O método mira guardar de 20% a 30% da renda no ano."
    >
      <p style={pStyle}>
        O Neko acompanha sua <b>Economia</b> contra essa meta e te avisa, com calma,
        quando o ritmo do ano está dentro ou fora do ideal. Sem punir, só mostrando.
      </p>
      <p style={pStyle}>
        Pronto. A partir daqui o app é seu. Bons lançamentos! <Sparkles size={14} />
      </p>
    </Step>
  );
}

/**
 * Onboarding de primeiro uso ("chá revelação") — 5 passos guiados que apresentam o método antes do
 * app. Persiste `onboarding_done` ao concluir/pular para não repetir. Tom calmo e amigável, com o
 * "wow" discreto da marca; respeita reduced-motion (animações CSS herdadas dos tokens).
 */
export function OnboardingFlow({
  onDone,
  onGoToSettings,
}: {
  onDone: () => void;
  onGoToSettings?: () => void;
}) {
  const [step, setStep] = useState(0);
  const [saving, setSaving] = useState(false);
  const cardRef = useRef<HTMLDivElement>(null);
  const dialogRef = useRef<HTMLDialogElement>(null);

  // `onDone` num ref para o listener de teclado (mount-only) ler a última versão sem entrar nas deps;
  // o ref é sincronizado num effect (nunca em render). `finish` persiste e fecha.
  const onDoneRef = useRef(onDone);
  useEffect(() => {
    onDoneRef.current = onDone;
  }, [onDone]);

  async function finish() {
    setSaving(true);
    try {
      await setAppSetting(ONBOARDING_KEY, "true");
    } catch {
      // Mesmo se a gravação falhar, não prendemos o usuário no onboarding.
    }
    onDone();
  }

  // Foca o cartão ao abrir, instala o focus-trap + Escape como listener imperativo no <dialog> (um
  // handler de teclado num container vai por addEventListener, não como prop num elemento não
  // interativo) e DEVOLVE o foco ao elemento anterior quando fecha.
  const previouslyFocused = useRef<HTMLElement | null>(null);
  useEffect(() => {
    previouslyFocused.current = document.activeElement as HTMLElement | null;
    cardRef.current?.focus();
    const dialog = dialogRef.current;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        void setAppSetting(ONBOARDING_KEY, "true").catch(() => undefined);
        onDoneRef.current();
        return;
      }
      if (e.key !== "Tab") return;
      const focusables = cardRef.current?.querySelectorAll<HTMLElement>(
        'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      );
      if (!focusables || focusables.length === 0) return;
      const first = focusables[0]!;
      const last = focusables[focusables.length - 1]!;
      const active = document.activeElement;
      if (e.shiftKey && (active === first || active === cardRef.current)) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      }
    };
    dialog?.addEventListener("keydown", handleKeyDown);
    return () => {
      dialog?.removeEventListener("keydown", handleKeyDown);
      const el = previouslyFocused.current;
      if (el?.isConnected) el.focus();
    };
  }, []);

  function next() {
    setStep((s) => Math.min(TOTAL_STEPS - 1, s + 1));
  }
  function back() {
    setStep((s) => Math.max(0, s - 1));
  }

  return (
    <dialog
      ref={dialogRef}
      open
      aria-modal="true"
      aria-label="Boas-vindas ao Neko Finance"
      style={OVERLAY_STYLE}
    >
      <div ref={cardRef} tabIndex={-1} style={CARD_STYLE}>
        <OnboardingHeader step={step} />

        <StepContent
          step={step}
          next={next}
          finish={() => void finish()}
          onGoToSettings={onGoToSettings}
        />

        {/* Navegação */}
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            gap: "var(--space-3)",
            marginTop: "var(--space-6)",
          }}
        >
          <Button variant="ghost" onClick={() => void finish()} disabled={saving}>
            Pular
          </Button>
          <div style={{ display: "flex", gap: "var(--space-2)" }}>
            {step > 0 && (
              <Button
                variant="secondary"
                iconLeft={<ArrowLeft size={15} strokeWidth={1.75} />}
                onClick={back}
              >
                Voltar
              </Button>
            )}
            {step < TOTAL_STEPS - 1 ? (
              <Button
                variant="primary"
                iconRight={<ArrowRight size={15} strokeWidth={1.75} />}
                onClick={next}
              >
                Avançar
              </Button>
            ) : (
              <Button
                variant="primary"
                iconRight={<Check size={15} strokeWidth={2} />}
                onClick={() => void finish()}
                disabled={saving}
              >
                Começar
              </Button>
            )}
          </div>
        </div>
      </div>
    </dialog>
  );
}

function Step({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle: string;
  children: React.ReactNode;
}) {
  return (
    <div style={{ marginTop: "var(--space-3)" }}>
      <h2
        style={{
          fontSize: "var(--fs-h2)",
          fontWeight: "var(--fw-bold)",
          letterSpacing: "var(--ls-tight)",
          margin: "var(--space-2) 0 var(--space-1)",
          color: "var(--text-strong)",
        }}
      >
        {title}
      </h2>
      <p style={{ color: "var(--text-muted)", fontSize: "var(--fs-sm)", margin: 0 }}>
        {subtitle}
      </p>
      {children}
    </div>
  );
}
