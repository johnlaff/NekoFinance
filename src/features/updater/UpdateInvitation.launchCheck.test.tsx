import { render, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("../../lib/env", async (importOriginal) => ({
  ...(await importOriginal()),
  isTauri: true,
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

describe("UpdateInvitation — checagem em background no launch (isTauri: true)", () => {
  it("checa uma vez no mount", async () => {
    const check = vi.fn().mockResolvedValue(null);
    const machine = createUpdaterMachine(fakeAdapter({ check }));

    render(<UpdateInvitation machine={machine} />);

    await waitFor(() => expect(check).toHaveBeenCalledTimes(1));
  });
});
