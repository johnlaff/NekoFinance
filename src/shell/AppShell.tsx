import { useEffect, useEffectEvent, useRef, useState } from "react";
import {
  CalendarDays,
  CalendarRange,
  ChartColumn,
  CreditCard,
  Ellipsis,
  House,
  List,
  Plus,
  Settings,
  Table2,
  Tags as TagsIcon,
  TrendingUp,
  Unlink,
} from "lucide-react";
import { NekoMark } from "../design-system/components/NekoMark";
import { ThemeToggle } from "./ThemeToggle";
import { lastSyncAt, type AuthStatus } from "../lib/api";
import { useCommand } from "../lib/useCommand";
import { syncRecencyLabel } from "../lib/syncRecency";

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

const SCREEN_META: Record<Screen, { title: string; crumb: string }> = {
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

interface NavItem {
  key: Screen;
  label: string;
  icon: typeof House | "cat";
}

/** Nav plana — cada destino no mesmo nível; a Mia leva o gato (o mesmo avatar do chat). */
const NAV_ITEMS: NavItem[] = [
  { key: "hoje", label: "Hoje", icon: House },
  { key: "lancamentos", label: "Lançamentos", icon: List },
  { key: "mes", label: "Este mês", icon: ChartColumn },
  { key: "cartoes", label: "Cartões", icon: CreditCard },
  { key: "ano", label: "O ano", icon: TrendingUp },
  { key: "calendario", label: "Calendário", icon: CalendarDays },
  { key: "horizonte", label: "Horizonte", icon: CalendarRange },
  { key: "tags", label: "Tags", icon: TagsIcon },
  { key: "mia", label: "Mia", icon: "cat" },
  { key: "config", label: "Configurações", icon: Settings },
];

/** Dock mobile: os 5 destinos do dia a dia; o resto vive no menu "mais" da appbar. */
const DOCK_KEYS: Screen[] = ["hoje", "lancamentos", "mes", "calendario", "mia"];
const MORE_KEYS: Screen[] = ["cartoes", "ano", "horizonte", "tags", "config"];

function NavIcon({ icon, size }: { icon: NavItem["icon"]; size: number }) {
  if (icon === "cat") return <NekoMark width={size} height={size} />;
  const Icon = icon;
  return <Icon size={size} strokeWidth={1.75} />;
}

/** Item da barra lateral. Dica numérica opcional (saldo de hoje, performance do mês). */
function NavButton({
  item,
  active,
  onNavigate,
  hint,
}: {
  item: NavItem;
  active: Screen;
  onNavigate: (screen: Screen) => void;
  hint?: string | undefined;
}) {
  return (
    <button
      type="button"
      className={`sh-item ${active === item.key ? "sh-item--active" : ""}`}
      aria-current={active === item.key ? "page" : undefined}
      // aria-label garante o nome no trilho tablet (rótulo display:none sai da
      // árvore de acessibilidade; sem isto o item da Mia herdaria o nome do SVG).
      aria-label={item.label}
      title={item.label}
      onClick={() => onNavigate(item.key)}
    >
      <span className="sh-item__ic" aria-hidden="true">
        <NavIcon icon={item.icon} size={19} />
      </span>
      <span className="sh-item__lbl">{item.label}</span>
      {hint ? <span className="sh-item__hint">{hint}</span> : null}
    </button>
  );
}

/** Menu "mais" da appbar mobile — destinos fora do dock. */
function MoreMenu({
  active,
  onNavigate,
}: {
  active: Screen;
  onNavigate: (screen: Screen) => void;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const firstItemRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    firstItemRef.current?.focus();
    const onPointerDown = (e: PointerEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node))
        setOpen(false);
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setOpen(false);
        triggerRef.current?.focus();
      }
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  return (
    <div className="sh-more" ref={rootRef}>
      {/* Popup simples de botões, não um role="menu": o padrão ARIA de menu
          exigiria setas + roving tabindex; Tab entre 4 botões é o contrato real. */}
      <button
        ref={triggerRef}
        type="button"
        className="sh-more__btn"
        aria-label="Mais telas"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <Ellipsis size={19} strokeWidth={1.75} />
      </button>
      {open && (
        <div className="sh-more__menu" role="group" aria-label="Mais telas">
          {MORE_KEYS.map((key, i) => {
            const item = NAV_ITEMS.find((n) => n.key === key);
            if (!item) return null;
            return (
              <button
                key={key}
                ref={i === 0 ? firstItemRef : undefined}
                type="button"
                className={`sh-more__item ${active === key ? "sh-more__item--active" : ""}`}
                onClick={() => {
                  setOpen(false);
                  onNavigate(key);
                  // A tela troca sob o foco; devolve-o ao gatilho para não cair no <body>.
                  triggerRef.current?.focus();
                }}
              >
                <NavIcon icon={item.icon} size={17} />
                {item.label}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

export function AppShell({
  active,
  onNavigate,
  authStatus,
  children,
  onCompose,
  hints,
  crumbs,
}: {
  active: Screen;
  onNavigate: (screen: Screen) => void;
  authStatus: AuthStatus;
  children: React.ReactNode;
  /** Abre o compositor de lançamento (CTA "Registrar lançamento" e atalho "N"). */
  onCompose?: () => void;
  /** Dicas numéricas da nav: { hoje: "R$ 27,17", mes: "R$ 1,2 mil" }. */
  hints?: Partial<Record<Screen, string>>;
  /** Crumb por tela sobrepondo o de `SCREEN_META` — ex.: a data de hoje na Hoje. */
  crumbs?: Partial<Record<Screen, string>>;
}) {
  const meta = SCREEN_META[active];
  const crumb = crumbs?.[active] ?? meta.crumb;
  const connected = authStatus === "connected";
  const bodyRef = useRef<HTMLDivElement>(null);
  // Dock encolhe ao rolar para baixo e volta ao subir (padrão de tab bar de app).
  const [dockMin, setDockMin] = useState(false);
  // Coordenação large-title: com [data-large-title] na tela, o título da appbar
  // só assume quando o título grande sai de vista.
  const [titleAssumed, setTitleAssumed] = useState(true);

  // Recência real da sincronização com a planilha (sync_log). Fica em silêncio
  // (fallback "Conta Google ativa") enquanto não há histórico ou dado carregado.
  const { data: lastSync } = useCommand("last_sync_at", lastSyncAt);
  const syncLabel = connected ? syncRecencyLabel(lastSync) : null;

  // Atalho "N" = novo lançamento (lê o callback mais recente via useEffectEvent; assina só no mount).
  const triggerCompose = useEffectEvent(() => onCompose?.());
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (
        e.key === "n" &&
        !e.metaKey &&
        !e.ctrlKey &&
        !e.altKey &&
        !(e.target instanceof HTMLInputElement) &&
        !(e.target instanceof HTMLTextAreaElement) &&
        !(e.target instanceof HTMLSelectElement)
      ) {
        e.preventDefault();
        triggerCompose();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  useEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    let last = body.scrollTop;
    const onScroll = () => {
      const y = body.scrollTop;
      // Histerese de 8px: micro-rolagens (bounce, ajuste de layout) não piscam o dock.
      if (y > last + 8 && y > 64) setDockMin(true);
      else if (y < last - 8 || y <= 64) setDockMin(false);
      last = y;
    };
    body.addEventListener("scroll", onScroll, { passive: true });
    return () => body.removeEventListener("scroll", onScroll);
  }, []);

  // react-doctor-disable-next-line react-doctor/effect-needs-cleanup -- os dois observers são desligados no retorno (mo.disconnect + io.disconnect); o observe vive num closure que a análise estática não rastreia
  useEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    // O .sh-body é o MESMO nó entre telas: sem reset, a tela nova herdaria o
    // scrollTop da anterior e o dock ficaria preso encolhido.
    body.scrollTop = 0;
    setDockMin(false);
    // O título grande pode montar DEPOIS do dado chegar (herói atrás de skeleton):
    // um MutationObserver troca o alvo observado quando o nó aparece ou some.
    const io = new IntersectionObserver(
      ([entry]) => setTitleAssumed(!(entry?.isIntersecting ?? false)),
      { root: body },
    );
    // Sentinel: o 1º bind SEMPRE processa — sem ele, tela nova sem herói faria
    // early-return (null === null) e herdaria o `quiet` da tela anterior.
    let current: Element | null | undefined = undefined;
    const bind = () => {
      const large = body.querySelector("[data-large-title]");
      if (large === current) return;
      if (current) io.unobserve(current);
      current = large;
      if (!large) {
        setTitleAssumed(true);
        return;
      }
      setTitleAssumed(false);
      io.observe(large);
    };
    bind();
    const mo = new MutationObserver(bind);
    mo.observe(body, { childList: true, subtree: true });
    return () => {
      mo.disconnect();
      io.disconnect();
    };
  }, [active]);

  return (
    <div className="sh">
      <aside className="sh-side">
        <div className="sh-brand">
          {/* Decorativo: o texto irmão "Neko" é o nome; sem isto o leitor anuncia 2×. */}
          <span className="sh-brand__cat" aria-hidden="true">
            <NekoMark width={19} height={19} />
          </span>
          <span className="sh-brand__name">Neko</span>
        </div>

        {/* aria-label fixa o nome acessível mesmo no trilho tablet, onde o rótulo some. */}
        <button
          type="button"
          className="sh-new"
          aria-label="Registrar lançamento (N)"
          title="Registrar lançamento (N)"
          onClick={() => onCompose?.()}
        >
          <span className="sh-new__lbl">Registrar lançamento</span>
          <kbd aria-hidden="true">N</kbd>
        </button>

        <nav className="sh-nav" aria-label="Navegação principal">
          {NAV_ITEMS.map((n) => (
            <NavButton
              key={n.key}
              item={n}
              active={active}
              onNavigate={onNavigate}
              hint={hints?.[n.key]}
            />
          ))}
        </nav>

        <div className="sh-foot">
          <ThemeToggle variant="row" />
          <button
            type="button"
            className="sh-conn"
            onClick={() => onNavigate("config")}
            title="Gerenciar conexão em Configurações"
          >
            <span className={`sh-conn__ic ${connected ? "" : "sh-conn__ic--off"}`}>
              {connected ? (
                <Table2 size={15} strokeWidth={1.75} />
              ) : (
                <Unlink size={15} strokeWidth={1.75} />
              )}
            </span>
            <span className="sh-conn__txt">
              <span className="sh-conn__t">
                {connected ? "Planilha conectada" : "Planilha"}
              </span>
              <span className="sh-conn__s">
                {authStatus === "connected"
                  ? syncLabel
                    ? `Sincronizada ${syncLabel}`
                    : "Conta Google ativa"
                  : authStatus === "expired"
                    ? "Sessão expirada"
                    : authStatus === "loading"
                      ? "Verificando…"
                      : "Desconectada"}
              </span>
            </span>
          </button>
        </div>
      </aside>

      <header className={`sh-appbar ${titleAssumed ? "" : "sh-appbar--quiet"}`}>
        <span className="sh-appbar__cat" aria-hidden="true">
          <NekoMark width={20} height={20} />
        </span>
        <div className="sh-appbar__title">
          <span className="sh-appbar__t">{meta.title}</span>
          <small>{crumb}</small>
        </div>
        <div className="sh-appbar__trail">
          <ThemeToggle />
          <MoreMenu active={active} onNavigate={onNavigate} />
        </div>
      </header>

      <main className="sh-main">
        <header className="sh-top">
          <div>
            <div className="sh-top__title">{meta.title}</div>
            <div className="sh-top__crumb">{crumb}</div>
          </div>
        </header>
        <div className="sh-body" ref={bodyRef}>
          <div className="sh-bodyinner">{children}</div>
        </div>
      </main>

      <nav
        className={`sh-dock ${dockMin ? "sh-dock--min" : ""}`}
        aria-label="Navegação do app"
        // Encolhido = invisível: sai do tab order inteiro, não só da vista.
        inert={dockMin || undefined}
      >
        <span className="sh-dock__tabs">
          {DOCK_KEYS.map((key) => {
            const item = NAV_ITEMS.find((n) => n.key === key);
            if (!item) return null;
            return (
              <button
                key={key}
                type="button"
                className={`sh-tab ${active === key ? "sh-tab--on" : ""}`}
                aria-current={active === key ? "page" : undefined}
                aria-label={item.label}
                onClick={() => onNavigate(key)}
              >
                <span aria-hidden="true">
                  <NavIcon icon={item.icon} size={21} />
                </span>
                <span aria-hidden="true">{item.label}</span>
              </button>
            );
          })}
        </span>
        <button
          type="button"
          className="sh-dock__fab"
          aria-label="Registrar lançamento"
          onClick={() => onCompose?.()}
        >
          <Plus size={22} strokeWidth={2} />
        </button>
      </nav>
    </div>
  );
}
