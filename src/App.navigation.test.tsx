import { render, screen } from "@testing-library/react";
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

describe("App navigation (redesign)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("groups the sidebar into Finanças and Sistema", async () => {
    mockAll();
    render(<App />);
    expect(await screen.findByText("Finanças")).toBeInTheDocument();
    expect(screen.getByText("Sistema")).toBeInTheDocument();
  });

  it("navigates to Lançamentos and marks it current", async () => {
    mockAll();
    const user = userEvent.setup();
    render(<App />);
    const item = await screen.findByRole("button", { name: "Lançamentos" });
    expect(item).not.toHaveAttribute("aria-current");
    await user.click(item);
    expect(item).toHaveAttribute("aria-current", "page");
  });

  it("opens the compose drawer from the Lançar button", async () => {
    mockAll();
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: /Lançar/ }));
    expect(screen.getByRole("dialog", { name: "Novo lançamento" })).toBeInTheDocument();
  });
});
