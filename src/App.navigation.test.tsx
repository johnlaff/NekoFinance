import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import App from "./App";
import { FORECAST, SUMMARY, TXNS, mockCommands, mockInvoke } from "./test/commands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

function mockAll() {
  mockCommands({
    check_auth_status: "disconnected",
    get_app_setting: "true",
    get_dashboard_summary: SUMMARY,
    get_forecast: FORECAST,
    get_recent_transactions: TXNS,
    get_upcoming_bills_cmd: [],
  });
}

/** Nav lateral (o dock mobile duplica os rótulos no DOM; o escopo desambigua). */
async function sideNav() {
  return within(await screen.findByRole("navigation", { name: "Navegação principal" }));
}

describe("App navigation (shell Midnight Purr)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("sidebar é nav plana — sem headers de grupo Finanças/Sistema", async () => {
    mockAll();
    render(<App />);
    const nav = await sideNav();
    expect(nav.getByRole("button", { name: /^Hoje/ })).toBeInTheDocument();
    expect(screen.queryByText("Finanças")).not.toBeInTheDocument();
    expect(screen.queryByText("Sistema")).not.toBeInTheDocument();
  });

  it("navigates to Lançamentos and marks it current", async () => {
    mockAll();
    const user = userEvent.setup();
    render(<App />);
    const nav = await sideNav();
    const item = nav.getByRole("button", { name: "Lançamentos" });
    expect(item).not.toHaveAttribute("aria-current");
    await user.click(item);
    expect(item).toHaveAttribute("aria-current", "page");
  });

  it("opens the compose drawer from the Registrar lançamento CTA", async () => {
    mockAll();
    const user = userEvent.setup();
    render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "Registrar lançamento (N)" }),
    );
    expect(screen.getByRole("dialog", { name: "Novo lançamento" })).toBeInTheDocument();
  });
});
