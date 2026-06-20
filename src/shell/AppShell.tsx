import { useEffect, useRef, useState } from "react";
import {
  Calculator,
  CalendarRange,
  HelpCircle,
  LayoutDashboard,
  Lock,
  Receipt,
  Search,
  Settings,
  Sparkles,
  Table2,
  Tags as TagsIcon,
  TrendingUp,
  Unlink,
} from "lucide-react";
import { NekoMark } from "../design-system/components/NekoMark";
import { ThemeToggle } from "./ThemeToggle";
import type { AuthStatus } from "../lib/api";

export type Screen =
  | "dashboard"
  | "totais"
  | "anuais"
  | "horizonte"
  | "transactions"
  | "tags"
  | "copilot"
  | "methodology"
  | "settings";

const SCREEN_META: Record<Screen, { title: string; crumb: string }> = {
  dashboard: { title: "Dashboard", crumb: "Quanto posso gastar hoje" },
  totais: { title: "Totais", crumb: "Cálculos do mês" },
  anuais: { title: "Visão anual", crumb: "O ano inteiro" },
  horizonte: { title: "Horizonte de saldos", crumb: "Projeção mês a mês" },
  transactions: { title: "Lançamentos", crumb: "Histórico completo" },
  tags: { title: "Tags", crumb: "Rótulos do mês" },
  copilot: { title: "Mia", crumb: "Copiloto" },
  methodology: { title: "Ajuda", crumb: "Como o Neko calcula" },
  settings: { title: "Configurações e privacidade", crumb: "Local · este dispositivo" },
};

interface NavItem {
  key: Screen;
  label: string;
  icon: typeof LayoutDashboard;
}

// Início = o que se toca em toda conferência noturna (<30s). Análise = visões diagnósticas.
const NAV_PRIMARY: NavItem[] = [
  { key: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { key: "transactions", label: "Lançamentos", icon: Receipt },
];

const NAV_ANALYSIS: NavItem[] = [
  { key: "totais", label: "Totais", icon: Calculator },
  { key: "anuais", label: "Anual", icon: TrendingUp },
  { key: "horizonte", label: "Horizonte", icon: CalendarRange },
  { key: "tags", label: "Tags", icon: TagsIcon },
];

/** Item de navegação da barra lateral. Vocabulário único reusado pelos três grupos. */
function NavButton({
  item,
  active,
  onNavigate,
}: {
  item: NavItem;
  active: Screen;
  onNavigate: (screen: Screen) => void;
}) {
  const Icon = item.icon;
  return (
    <button
      type="button"
      className={`ak-item ${active === item.key ? "ak-item--active" : ""}`}
      aria-current={active === item.key ? "page" : undefined}
      onClick={() => onNavigate(item.key)}
    >
      <Icon size={18} strokeWidth={1.75} className="ak-item__ic" />
      <span>{item.label}</span>
    </button>
  );
}

export function AppShell({
  active,
  onNavigate,
  onSearch,
  authStatus,
  children,
}: {
  active: Screen;
  onNavigate: (screen: Screen) => void;
  onSearch: (query: string) => void;
  authStatus: AuthStatus;
  children: React.ReactNode;
}) {
  const [searchDraft, setSearchDraft] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const meta = SCREEN_META[active];
  const connected = authStatus === "connected";
  const isMac =
    typeof navigator !== "undefined" && /mac/i.test(navigator.platform ?? "");

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <div className="ak">
      <aside className="ak-side">
        <div className="ak-brand">
          <span className="ak-brand__mark">
            <NekoMark width={26} height={26} />
          </span>
          <span className="ak-brand__name">Neko</span>
          <span className="ak-brand__tag">
            <Lock size={11} strokeWidth={1.75} />
            Local
          </span>
        </div>

        <nav className="ak-nav" aria-label="Navegação principal">
          <div className="ak-navh">Início</div>
          {NAV_PRIMARY.map((n) => (
            <NavButton key={n.key} item={n} active={active} onNavigate={onNavigate} />
          ))}

          <div className="ak-navh">Análise</div>
          {NAV_ANALYSIS.map((n) => (
            <NavButton key={n.key} item={n} active={active} onNavigate={onNavigate} />
          ))}

          <div className="ak-navh">Sistema</div>
          <NavButton
            item={{
              key: "settings",
              label: "Configurações e privacidade",
              icon: Settings,
            }}
            active={active}
            onNavigate={onNavigate}
          />
          {/* Metodologia rebaixada: doc estático acessível por "Ajuda", não mais um par das telas do dia. */}
          <NavButton
            item={{ key: "methodology", label: "Ajuda", icon: HelpCircle }}
            active={active}
            onNavigate={onNavigate}
          />
          {/* Mia ainda é um stub ("Em desenvolvimento") → entra aqui, não compete com as telas diárias. */}
          <NavButton
            item={{ key: "copilot", label: "Mia", icon: Sparkles }}
            active={active}
            onNavigate={onNavigate}
          />
        </nav>

        <div className="ak-side__foot">
          <button
            type="button"
            className="ak-conn ak-conn--btn"
            onClick={() => onNavigate("settings")}
            title="Gerenciar conexão em Configurações"
          >
            <span className={`ak-conn__ic ${connected ? "" : "ak-conn__ic--off"}`}>
              {connected ? (
                <Table2 size={14} strokeWidth={1.75} />
              ) : (
                <Unlink size={14} strokeWidth={1.75} />
              )}
            </span>
            <span className="ak-conn__txt">
              <span className="ak-conn__t">Google Sheets</span>
              <span className="ak-conn__s">
                {authStatus === "connected"
                  ? "Conectado"
                  : authStatus === "expired"
                    ? "Sessão expirada"
                    : authStatus === "loading"
                      ? "Verificando…"
                      : "Desconectado"}
              </span>
            </span>
          </button>
        </div>
      </aside>

      <main className="ak-main">
        <header className="ak-top">
          <div className="ak-top__titles">
            <div className="ak-top__title">{meta.title}</div>
            <div className="ak-top__crumb">{meta.crumb}</div>
          </div>
          <div className="ak-spacer" />
          <form
            className="ak-search"
            role="search"
            onSubmit={(e) => {
              e.preventDefault();
              onSearch(searchDraft);
            }}
          >
            <Search size={15} strokeWidth={1.75} />
            <input
              ref={searchRef}
              aria-label="Buscar lançamentos"
              placeholder="Buscar lançamentos…"
              type="search"
              value={searchDraft}
              onChange={(e) => setSearchDraft(e.target.value)}
            />
            <kbd className="ak-kbd" aria-hidden="true">
              {isMac ? "⌘K" : "Ctrl K"}
            </kbd>
          </form>
          <ThemeToggle />
        </header>
        <div className="ak-body">{children}</div>
      </main>
    </div>
  );
}
