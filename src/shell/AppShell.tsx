import { useEffect, useEffectEvent } from "react";
import {
  Bell,
  Calculator,
  CalendarRange,
  LayoutDashboard,
  LayoutList,
  Lock,
  Plus,
  Receipt,
  Settings,
  Sparkles,
  Table2,
  Tags as TagsIcon,
  TrendingUp,
  Unlink,
} from "lucide-react";
import { NekoMark } from "../design-system/components/NekoMark";
import { Button } from "../design-system/components/Button";
import { ThemeToggle } from "./ThemeToggle";
import type { AuthStatus } from "../lib/api";

/** Nova IA (redesign 2026): cada item = uma pergunta/objetivo único. */
export type Screen =
  | "hoje"
  | "lancamentos"
  | "mes"
  | "ano"
  | "calendario"
  | "horizonte"
  | "tags"
  | "mia"
  | "config";

const SCREEN_META: Record<Screen, { title: string; crumb: string }> = {
  hoje: { title: "Hoje", crumb: "Quanto posso gastar hoje" },
  lancamentos: { title: "Lançamentos", crumb: "Seu livro-razão" },
  mes: { title: "Este mês", crumb: "Como o mês está indo" },
  ano: { title: "O ano", crumb: "O ano num olhar" },
  calendario: { title: "Calendário", crumb: "Saúde do saldo dia a dia" },
  horizonte: { title: "Horizonte", crumb: "Para onde o saldo vai" },
  tags: { title: "Tags", crumb: "Gasto por tag" },
  mia: { title: "Mia", crumb: "Sua copilota financeira" },
  config: { title: "Configurações", crumb: "Conexão e privacidade" },
};

interface NavItem {
  key: Screen;
  label: string;
  icon: typeof LayoutDashboard;
}

const NAV_FINANCAS: NavItem[] = [
  { key: "hoje", label: "Hoje", icon: LayoutDashboard },
  { key: "lancamentos", label: "Lançamentos", icon: Receipt },
  { key: "mes", label: "Este mês", icon: Calculator },
  { key: "ano", label: "O ano", icon: TrendingUp },
  { key: "calendario", label: "Calendário", icon: LayoutList },
  { key: "horizonte", label: "Horizonte", icon: CalendarRange },
  { key: "tags", label: "Tags", icon: TagsIcon },
];

const NAV_SISTEMA: NavItem[] = [
  { key: "mia", label: "Mia", icon: Sparkles },
  { key: "config", label: "Configurações", icon: Settings },
];

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
  const Icon = item.icon;
  return (
    <button
      type="button"
      className={`sh-item ${active === item.key ? "sh-item--active" : ""}`}
      aria-current={active === item.key ? "page" : undefined}
      onClick={() => onNavigate(item.key)}
    >
      <Icon size={18} strokeWidth={1.75} className="sh-item__ic" />
      <span>{item.label}</span>
      {hint ? <span className="sh-item__hint">{hint}</span> : null}
    </button>
  );
}

export function AppShell({
  active,
  onNavigate,
  authStatus,
  children,
  onCompose,
  hints,
}: {
  active: Screen;
  onNavigate: (screen: Screen) => void;
  authStatus: AuthStatus;
  children: React.ReactNode;
  /** Abre o compositor de lançamento (botão "Lançar" e atalho "N"). */
  onCompose?: () => void;
  /** Dicas numéricas da nav: { hoje: "R$ 27,17", mes: "R$ 1,2 mil" }. */
  hints?: Partial<Record<Screen, string>>;
}) {
  const meta = SCREEN_META[active];
  const connected = authStatus === "connected";

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

  return (
    <div className="sh">
      <aside className="sh-side">
        <div className="sh-brand">
          <span className="sh-brand__mark">
            <NekoMark width={30} height={30} />
          </span>
          <span className="sh-brand__name">Neko</span>
          <span className="sh-brand__tag">
            <Lock size={10} strokeWidth={2} />
            Local
          </span>
        </div>

        <nav className="sh-nav" aria-label="Navegação principal">
          <div className="sh-navh">Finanças</div>
          {NAV_FINANCAS.map((n) => (
            <NavButton
              key={n.key}
              item={n}
              active={active}
              onNavigate={onNavigate}
              hint={hints?.[n.key]}
            />
          ))}
          <div className="sh-navh">Sistema</div>
          {NAV_SISTEMA.map((n) => (
            <NavButton key={n.key} item={n} active={active} onNavigate={onNavigate} />
          ))}
        </nav>

        <div className="sh-foot">
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
            <span>
              <span className="sh-conn__t" style={{ display: "block" }}>
                {connected ? "Planilha conectada" : "Planilha"}
              </span>
              <span className="sh-conn__s">
                {authStatus === "connected"
                  ? "Sincronizada há 2 min"
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

      <main className="sh-main">
        <header className="sh-top">
          <div>
            <div className="sh-top__title">{meta.title}</div>
            <div className="sh-top__crumb">{meta.crumb}</div>
          </div>
          <div className="sh-spacer" />
          <Button
            size="sm"
            variant="primary"
            iconLeft={<Plus size={15} strokeWidth={2} />}
            onClick={() => onCompose?.()}
          >
            Lançar
          </Button>
          <ThemeToggle />
          <button type="button" className="sh-iconbtn" aria-label="Notificações">
            <Bell size={17} strokeWidth={1.75} />
          </button>
        </header>
        <div className="sh-body">
          <div className="sh-bodyinner">{children}</div>
        </div>
      </main>
    </div>
  );
}
