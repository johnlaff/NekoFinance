/**
 * HealthBadge — pill de status com anel de progresso (o "Sobrou dinheiro" / "Dentro da renda"
 * do método). Em inline-style (convenção do Neko) em vez
 * do hook de injeção de CSS → puro, sem hooks/estado/efeito (React Doctor não aplicável).
 *
 * `level` define a cor/rótulo padrão; `label` sobrescreve com o texto do método ("Sobrou dinheiro",
 * "Dentro do ideal", "Dentro da renda"…). O anel anima via CSS transition respeitando
 * prefers-reduced-motion (a transição some quando o usuário pede menos movimento).
 */
export type HealthLevel = "strong" | "steady" | "watch" | "risk";

const TONE: Record<HealthLevel, { bg: string; border: string; color: string }> = {
  strong: {
    bg: "var(--success-tint)",
    border: "color-mix(in srgb, var(--success-400) 25%, transparent)",
    color: "var(--success-400)",
  },
  steady: {
    bg: "var(--primary-quiet)",
    border: "color-mix(in srgb, var(--primary) 22%, transparent)",
    color: "var(--primary)",
  },
  watch: {
    bg: "var(--warning-tint)",
    border: "color-mix(in srgb, var(--warning-400) 25%, transparent)",
    color: "var(--warning-400)",
  },
  risk: {
    bg: "var(--danger-tint)",
    border: "color-mix(in srgb, var(--danger-400) 25%, transparent)",
    color: "var(--danger-400)",
  },
};

const DEFAULT_LABEL: Record<HealthLevel, string> = {
  strong: "Forte",
  steady: "Estável",
  watch: "Atenção",
  risk: "Em risco",
};

const DEFAULT_SCORE: Record<HealthLevel, number> = {
  strong: 92,
  steady: 74,
  watch: 48,
  risk: 24,
};

interface HealthBadgeProps {
  level?: HealthLevel;
  /** Sobrescreve o rótulo padrão (ex.: "Sobrou dinheiro", "Dentro da renda"). */
  label?: string;
  sublabel?: string;
  /** 0–100 para o anel; default por nível. */
  score?: number;
  size?: "md" | "lg";
  className?: string;
}

export function HealthBadge({
  level = "steady",
  label,
  sublabel,
  score,
  size = "md",
  className = "",
}: HealthBadgeProps) {
  const tone = TONE[level];
  const text = label ?? DEFAULT_LABEL[level];
  const pct = score ?? DEFAULT_SCORE[level];
  const dim = size === "lg" ? 34 : 24;
  const r = size === "lg" ? 15 : 10;
  const c = 2 * Math.PI * r;
  const cx = dim / 2;
  return (
    <span
      role="img"
      aria-label={sublabel ? `${text} — ${sublabel}` : text}
      className={className}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "10px",
        padding: size === "lg" ? "10px 18px 10px 12px" : "7px 13px 7px 9px",
        borderRadius: "var(--radius-pill)",
        fontFamily: "var(--font-sans)",
        border: `1px solid ${tone.border}`,
        background: tone.bg,
        color: tone.color,
        lineHeight: 1,
      }}
    >
      <svg
        aria-hidden="true"
        width={dim}
        height={dim}
        viewBox={`0 0 ${dim} ${dim}`}
        style={{ flex: "none", transform: "rotate(-90deg)" }}
      >
        <circle
          cx={cx}
          cy={cx}
          r={r}
          fill="none"
          stroke="currentColor"
          strokeWidth="3"
          opacity="0.2"
        />
        <circle
          cx={cx}
          cy={cx}
          r={r}
          fill="none"
          stroke="currentColor"
          strokeWidth="3"
          strokeLinecap="round"
          strokeDasharray={c}
          strokeDashoffset={c * (1 - pct / 100)}
          style={{
            transition: "stroke-dashoffset var(--dur-slow) var(--ease-entrance)",
          }}
        />
      </svg>
      <span style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
        <span
          style={{
            fontSize: size === "lg" ? "var(--fs-title)" : "var(--fs-sm)",
            fontWeight: "var(--fw-bold)",
            letterSpacing: "-0.005em",
          }}
        >
          {text}
        </span>
        {sublabel ? (
          <span
            style={{
              fontSize: "var(--fs-micro)",
              fontWeight: "var(--fw-medium)",
              opacity: 0.8,
            }}
          >
            {sublabel}
          </span>
        ) : null}
      </span>
    </span>
  );
}
