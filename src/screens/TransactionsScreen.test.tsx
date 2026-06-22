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
    // A primeira transação do fixture deve aparecer no livro-razão.
    expect(await screen.findByText(TXNS[0]!.description)).toBeInTheDocument();
  });
});
