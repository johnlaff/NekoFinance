import { render, screen } from "@testing-library/react";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { TransactionsScreen } from "./TransactionsScreen";
import { NekoAppProvider } from "../shell/appContext";
import { TXNS, mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const app = { navigate: vi.fn(), openCompose: vi.fn() };

function renderLedger() {
  return render(
    <NekoAppProvider value={app}>
      <TransactionsScreen />
    </NekoAppProvider>,
  );
}

describe("TransactionsScreen (Lançamentos)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("renders the ledger with the loaded transactions", async () => {
    mockCommands({ get_recent_transactions: TXNS });
    renderLedger();
    // O default "Por mês" mostra o mês atual do fixture.
    expect(await screen.findByText(TXNS[2]!.description)).toBeInTheDocument();
  });

  it("opens in Por mês view by default and lists it first", async () => {
    mockCommands({ get_recent_transactions: TXNS });
    renderLedger();

    const month = await screen.findByRole("radio", { name: "Por mês" });
    const timeline = screen.getByRole("radio", { name: "Linha do tempo" });

    expect(month).toHaveAttribute("aria-checked", "true");
    expect(timeline).toHaveAttribute("aria-checked", "false");
    expect(
      month.compareDocumentPosition(timeline) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });
});
