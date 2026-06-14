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

  it("navigates to Transações and marks it current", async () => {
    const user = userEvent.setup();
    render(<App />);

    const navItem = screen.getByRole("button", { name: "Transações" });
    expect(navItem).not.toHaveAttribute("aria-current");
    await user.click(navItem);

    expect(navItem).toHaveAttribute("aria-current", "page");
    await waitFor(() => {
      expect(screen.getByText(/exibidas?/)).toBeInTheDocument();
    });
    expect(screen.getByLabelText("Filtrar por descrição")).toBeInTheDocument();
  });

  it("navigates to Metodologia", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "Metodologia" }));
    expect(screen.getByText(/Previsibilidade primeiro/)).toBeInTheDocument();
    expect(screen.getByText("Régua 1 e Régua 2")).toBeInTheDocument();
  });

  it("navigates to Mia via sidebar and via dashboard button", async () => {
    const user = userEvent.setup();
    render(<App />);

    const sidebarItem = () =>
      screen
        .getAllByRole("button", { name: "Perguntar à Mia" })
        .find((b) => b.classList.contains("ak-item"));
    await user.click(sidebarItem()!);
    expect(screen.getByText("O que a Mia vai fazer")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Dashboard" }));
    await waitFor(() => {
      expect(screen.getByText("Pode gastar até")).toBeInTheDocument();
    });
    const heroButton = screen
      .getAllByRole("button", { name: "Perguntar à Mia" })
      .find((b) => !b.classList.contains("ak-item"));
    await user.click(heroButton!);
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

  it("header search lands on Transações with the query applied", async () => {
    const user = userEvent.setup();
    render(<App />);

    const search = screen.getByLabelText("Buscar transações");
    await user.type(search, "mercado{Enter}");

    await waitFor(() => {
      expect(screen.getByLabelText("Filtrar por descrição")).toHaveValue("mercado");
    });
    expect(screen.getByText("Café + mercado")).toBeInTheDocument();
    expect(screen.queryByText("Streaming anual")).not.toBeInTheDocument();
  });
});
