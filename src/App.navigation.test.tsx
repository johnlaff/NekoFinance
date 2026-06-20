import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import App from "./App";
import {
  APP_INFO,
  FORECAST,
  SUMMARY,
  TXNS,
  mockCommands,
  mockInvoke,
} from "./test/commands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

function mockAll() {
  mockCommands({
    check_auth_status: "disconnected",
    get_dashboard_summary: SUMMARY,
    get_forecast: FORECAST,
    get_recent_transactions: TXNS,
    get_app_info: APP_INFO,
  });
}

describe("App navigation", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockAll();
  });

  it("navigates to Lançamentos and marks it current", async () => {
    const user = userEvent.setup();
    render(<App />);

    const navItem = screen.getByRole("button", { name: "Lançamentos" });
    expect(navItem).not.toHaveAttribute("aria-current");
    await user.click(navItem);

    expect(navItem).toHaveAttribute("aria-current", "page");
    await waitFor(() => {
      expect(screen.getByText(/exibidas?/)).toBeInTheDocument();
    });
    // Não há mais busca própria na tela: a única afordância é o ⌘K do header.
    expect(screen.getByLabelText("Buscar lançamentos")).toBeInTheDocument();
    expect(screen.queryByLabelText("Filtrar por descrição")).not.toBeInTheDocument();
  });

  it("sidebar groups items into Início and Análise", () => {
    render(<App />);
    expect(screen.getByText("Início")).toBeInTheDocument();
    expect(screen.getByText("Análise")).toBeInTheDocument();
    expect(screen.getByText("Sistema")).toBeInTheDocument();
  });

  it("navigates to a Metodologia via the demoted Ajuda entry", async () => {
    const user = userEvent.setup();
    render(<App />);

    // Metodologia foi rebaixada: chega-se a ela pelo item "Ajuda" em Sistema, não como par das telas do dia.
    expect(
      screen.queryByRole("button", { name: "Metodologia" }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Ajuda" }));
    expect(screen.getByText(/Previsibilidade primeiro/)).toBeInTheDocument();
    expect(screen.getByText("Débito e crédito: dois ritmos")).toBeInTheDocument();
  });

  it("navigates to Mia via sidebar and via dashboard button", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "Mia" }));
    expect(screen.getByText("O que a Mia vai fazer")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Dashboard" }));
    await waitFor(() => {
      expect(screen.getByText("Pode gastar até")).toBeInTheDocument();
    });
    // O CTA do dashboard mantém "Conhecer a Mia" (convite); a nav lateral é só "Mia".
    await user.click(screen.getByRole("button", { name: "Conhecer a Mia" }));
    expect(screen.getByText("O que a Mia vai fazer")).toBeInTheDocument();
  });

  it("navigates to Configurações e privacidade", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(
      screen.getByRole("button", { name: "Configurações e privacidade" }),
    );
    await waitFor(() => {
      expect(screen.getByText(APP_INFO.db_path)).toBeInTheDocument();
    });
    expect(screen.getByText("Importar arquivo local")).toBeInTheDocument();
  });

  it("header search lands on Lançamentos with the query applied", async () => {
    const user = userEvent.setup();
    render(<App />);

    const search = screen.getByLabelText("Buscar lançamentos");
    await user.type(search, "variável{Enter}");

    // A busca do header é a única afordância: a lista já chega filtrada (sem input local duplicado).
    await waitFor(() => {
      expect(screen.getByText("Despesa demo variável")).toBeInTheDocument();
    });
    expect(screen.getByText("Busca:")).toBeInTheDocument();
    expect(screen.queryByText("Compromisso demo no crédito")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Filtrar por descrição")).not.toBeInTheDocument();
  });
});
