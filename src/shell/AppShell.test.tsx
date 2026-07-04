import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { AppShell } from "./AppShell";
import { mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

function renderShell() {
  render(
    <AppShell active="lancamentos" onNavigate={vi.fn()} authStatus="connected">
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
