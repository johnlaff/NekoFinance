import { render, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("../../lib/env", async (importOriginal) => ({
  ...(await importOriginal()),
  isTauri: true,
  isAndroid: true,
}));

import { UpdateInvitation } from "./UpdateInvitation";
import { createUpdaterMachine, type UpdaterAdapter } from "./updaterView";

function fakeAdapter(overrides: Partial<UpdaterAdapter> = {}): UpdaterAdapter {
  return {
    check: vi.fn().mockResolvedValue(null),
    checkSpace: vi.fn().mockResolvedValue({
      ok: true,
      required_bytes: 0,
      free_bytes: 0,
      missing_bytes: 0,
    }),
    relaunch: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

describe("UpdateInvitation — Android (isAndroid: true)", () => {
  it("nunca checa no mount — o plugin não existe nesta plataforma", async () => {
    const check = vi.fn();
    const machine = createUpdaterMachine(fakeAdapter({ check }));

    render(<UpdateInvitation machine={machine} />);

    await waitFor(() => expect(machine.getState()).toEqual({ status: "idle" }));
    expect(check).not.toHaveBeenCalled();
  });
});
