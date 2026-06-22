import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import App from "./App";
import { FORECAST, SUMMARY, TXNS, mockCommands, mockInvoke } from "./test/commands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("App (redesign)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("renders the Hoje screen with loaded data", async () => {
    mockCommands({
      check_auth_status: "disconnected",
      get_app_setting: "true",
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_recent_transactions: TXNS,
      get_upcoming_bills_cmd: [],
    });
    render(<App />);
    expect(await screen.findByText("Pode gastar hoje")).toBeInTheDocument();
    expect(screen.getByText("Check-in de hoje")).toBeInTheDocument();
  });
});
