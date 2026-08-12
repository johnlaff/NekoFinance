import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("../../lib/env", async (importOriginal) => ({
  ...(await importOriginal()),
  isAndroid: true,
}));

import { UpdateSettingsBlock } from "./UpdateSettingsBlock";
import { createUpdaterMachine, type UpdaterAdapter } from "./updaterView";

function fakeAdapter(): UpdaterAdapter {
  return {
    check: vi.fn().mockResolvedValue(null),
    checkSpace: vi.fn().mockResolvedValue({
      ok: true,
      required_bytes: 0,
      free_bytes: 0,
      missing_bytes: 0,
    }),
    relaunch: vi.fn().mockResolvedValue(undefined),
  };
}

describe("UpdateSettingsBlock — Android (isAndroid: true)", () => {
  it("mostra indisponibilidade honesta, sem convite a checar", () => {
    const check = vi.fn();
    const machine = createUpdaterMachine({ ...fakeAdapter(), check });
    render(<UpdateSettingsBlock machine={machine} />);

    expect(
      screen.getByText(
        "Atualização automática não está disponível no Android — instale a versão mais nova pelo ADB.",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText("Nenhuma atualização pendente")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Verificar agora" }),
    ).not.toBeInTheDocument();
    expect(check).not.toHaveBeenCalled();
  });
});
