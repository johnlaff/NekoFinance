import { useEffect, useRef, useState } from "react";
import {
  BookOpen,
  Calculator,
  CalendarRange,
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

export const SCREEN_META: Record<Screen, { title: string; crumb: string }> = {
  dashboard: { title: "Dashboard", crumb: "Todas as contas" },
  totais: { title: "Totais", crumb: "Cálculos do mês" },
  anuais: { title: "Visão anual", crumb: "O ano inteiro" },
  horizonte: { title: "Horizonte de saldos", crumb: "Projeção mês a mês" },
  transactions: { title: "Lançamentos", crumb: "Histórico completo" },
  tags: { title: "Tags", crumb: "Rótulos do mês" },
  copilot: { title: "Mia", crumb: "Copiloto" },
  methodology: { title: "Metodologia", crumb: "Como o Neko calcula" },
  settings: { title: "Configurações e privacidade", crumb: "Local · este dispositivo" },
};

const NAV_ITEMS: { key: Screen; label: string; icon: typeof LayoutDashboard }[] = [
  { key: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { key: "totais", label: "Totais", icon: Calculator },
  { key: "anuais", label: "Anual", icon: TrendingUp },
  { key: "horizonte", label: "Horizonte", icon: CalendarRange },
  { key: "transactions", label: "Lançamentos", icon: Receipt },
  { key: "tags", label: "Tags", icon: TagsIcon },
  { key: "copilot", label: "Mia", icon: Sparkles },
  { key: "methodology", label: "Metodologia", icon: BookOpen },
];

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

        <nav className="ak-nav">
          <div className="ak-navh">Finanças</div>
          {NAV_ITEMS.map((n) => (
            <button
              key={n.key}
              type="button"
              className={`ak-item ${active === n.key ? "ak-item--active" : ""}`}
              aria-current={active === n.key ? "page" : undefined}
              onClick={() => onNavigate(n.key)}
            >
              <n.icon size={18} strokeWidth={1.75} className="ak-item__ic" />
              <span>{n.label}</span>
            </button>
          ))}

          <div className="ak-navh">Sistema</div>
          <button
            type="button"
            className={`ak-item ${active === "settings" ? "ak-item--active" : ""}`}
            aria-current={active === "settings" ? "page" : undefined}
            onClick={() => onNavigate("settings")}
          >
            <Settings size={18} strokeWidth={1.75} className="ak-item__ic" />
            <span>Configurações e privacidade</span>
          </button>
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

      <div className="ak-main">
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
      </div>
    </div>
  );
}
