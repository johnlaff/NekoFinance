import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { YearGridScreen } from "./YearGridScreen";
import { FORECAST, mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("YearGridScreen (Calendário)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("renders the calendar with the saldo thermometer legend", async () => {
    mockCommands({ get_forecast: FORECAST });
    render(<YearGridScreen />);
    expect(await screen.findByText("Folga")).toBeInTheDocument();
  });
});
