import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { AppShell } from "./AppShell";
import type { Screen } from "./screens";
import { mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

function renderShell(overrides: { onNavigate?: (s: Screen) => void } = {}) {
  render(
    <AppShell
      active="lancamentos"
      onNavigate={overrides.onNavigate ?? vi.fn()}
      authStatus="connected"
    >
      <div />
    </AppShell>,
  );
}

describe("AppShell — recência real de sync", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    // Relógio congelado em UTC; o timestamp do sync_log também é UTC.
    vi.setSystemTime(new Date("2026-07-04T12:00:00Z"));
    mockInvoke.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("mostra a recência real quando há histórico (parse UTC do datetime('now'))", async () => {
    // "2026-07-04 11:50:00" (UTC, sem sufixo, como o SQLite grava) = 10 min atrás.
    mockCommands({ last_sync_at: "2026-07-04 11:50:00" });
    renderShell();

    expect(await screen.findByText("Sincronizada há 10 min")).toBeInTheDocument();
  });

  it("cai para 'Conta Google ativa' quando não há histórico", async () => {
    mockCommands({ last_sync_at: null });
    renderShell();

    expect(await screen.findByText("Conta Google ativa")).toBeInTheDocument();
    expect(screen.queryByText(/Sincronizada/)).not.toBeInTheDocument();
  });
});

describe("AppShell — shell por viewport (todos os destinos alcançáveis)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockCommands({ last_sync_at: null });
  });

  it("sidebar tem nav plana com os 9 destinos, sem headers de grupo", () => {
    renderShell();
    const nav = screen.getByRole("navigation", { name: "Navegação principal" });
    const labels = within(nav)
      .getAllByRole("button")
      .map((b) => b.textContent);
    expect(labels).toEqual([
      "Hoje",
      "Lançamentos",
      "Este mês",
      "Cartões",
      "O ano",
      "Calendário",
      "Horizonte",
      "Tags",
      "Mia",
      "Configurações",
    ]);
    expect(screen.queryByText("Finanças")).not.toBeInTheDocument();
    expect(screen.queryByText("Sistema")).not.toBeInTheDocument();
  });

  it("dock mobile tem os 5 destinos do dia a dia + FAB de registrar", () => {
    const onCompose = vi.fn();
    render(
      <AppShell
        active="hoje"
        onNavigate={vi.fn()}
        authStatus="connected"
        onCompose={onCompose}
      >
        <div />
      </AppShell>,
    );
    const dock = screen.getByRole("navigation", { name: "Navegação do app" });
    const tabs = within(dock)
      .getAllByRole("button")
      .map((b) => b.textContent || b.getAttribute("aria-label"));
    expect(tabs).toEqual([
      "Hoje",
      "Lançamentos",
      "Este mês",
      "Calendário",
      "Mia",
      "Registrar lançamento",
    ]);
    within(dock).getByRole("button", { name: "Registrar lançamento" }).click();
    expect(onCompose).toHaveBeenCalledTimes(1);
  });

  it("menu “mais” da appbar navega para os destinos fora do dock", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    renderShell({ onNavigate });

    await user.click(screen.getByRole("button", { name: "Mais telas" }));
    const menu = screen.getByRole("group", { name: "Mais telas" });
    const items = within(menu)
      .getAllByRole("button")
      .map((b) => b.textContent);
    expect(items).toEqual(["Cartões", "O ano", "Horizonte", "Tags", "Configurações"]);

    await user.click(within(menu).getByRole("button", { name: "Horizonte" }));
    expect(onNavigate).toHaveBeenCalledWith("horizonte");
    expect(screen.queryByRole("group", { name: "Mais telas" })).not.toBeInTheDocument();
  });

  it("CTA da sidebar dispara o compositor", () => {
    const onCompose = vi.fn();
    render(
      <AppShell
        active="hoje"
        onNavigate={vi.fn()}
        authStatus="connected"
        onCompose={onCompose}
      >
        <div />
      </AppShell>,
    );
    screen.getByRole("button", { name: "Registrar lançamento (N)" }).click();
    expect(onCompose).toHaveBeenCalledTimes(1);
  });

  it("crumb por tela sobrepõe o de SCREEN_META (a data da Hoje)", () => {
    render(
      <AppShell
        active="hoje"
        onNavigate={vi.fn()}
        authStatus="connected"
        crumbs={{ hoje: "Quarta-feira, 15 de julho" }}
      >
        <div />
      </AppShell>,
    );
    expect(screen.getAllByText("Quarta-feira, 15 de julho").length).toBeGreaterThan(0);
    expect(screen.queryByText("Quanto posso gastar hoje")).not.toBeInTheDocument();
  });
});

describe("AppShell — coordenação large-title", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockCommands({ last_sync_at: null });
  });

  // O stub global de IntersectionObserver (setup.ts) é no-op: o contrato testado
  // aqui é o BIND (quiet imediato quando o título grande aparece), não o scroll.
  it("título grande montado DEPOIS do dado chegar ainda aquieta a appbar", async () => {
    const { container, rerender } = render(
      <AppShell active="hoje" onNavigate={vi.fn()} authStatus="connected">
        <div />
      </AppShell>,
    );
    const appbar = container.querySelector(".sh-appbar")!;
    // Sem título grande na tela, a appbar assume o título direto.
    expect(appbar.className).not.toContain("sh-appbar--quiet");

    // O herói data-gated monta num render posterior (skeleton → dado):
    rerender(
      <AppShell active="hoje" onNavigate={vi.fn()} authStatus="connected">
        <section data-large-title>
          <h1>Boa noite.</h1>
        </section>
      </AppShell>,
    );
    await vi.waitFor(() => {
      expect(appbar.className).toContain("sh-appbar--quiet");
    });
  });
});
