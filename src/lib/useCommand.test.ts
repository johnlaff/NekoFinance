import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invalidateCommands, useCommand } from "./useCommand";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("useCommand", () => {
  beforeEach(() => {
    invalidateCommands();
  });

  it("loads on first mount and resolves data", async () => {
    const fetcher = vi.fn().mockResolvedValue({ n: 1 });
    const { result } = renderHook(() => useCommand("cmd_a", fetcher));

    expect(result.current.loading).toBe(true);
    await waitFor(() => {
      expect(result.current.data).toEqual({ n: 1 });
    });
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("serves the cached value synchronously on remount (no skeleton flash)", async () => {
    const fetcher = vi.fn().mockResolvedValue("fresh");
    const first = renderHook(() => useCommand("cmd_b", fetcher));
    await waitFor(() => {
      expect(first.result.current.data).toBe("fresh");
    });
    first.unmount();

    const second = renderHook(() => useCommand("cmd_b", fetcher));
    // Cached value is available before any async work resolves.
    expect(second.result.current.data).toBe("fresh");
    expect(second.result.current.loading).toBe(false);
    await waitFor(() => {
      expect(fetcher).toHaveBeenCalledTimes(2); // still revalidates in background
    });
  });

  it("surfaces errors while keeping any cached data", async () => {
    const fetcher = vi
      .fn()
      .mockResolvedValueOnce("ok")
      .mockRejectedValueOnce(new Error("boom"));
    const first = renderHook(() => useCommand("cmd_c", fetcher));
    await waitFor(() => {
      expect(first.result.current.data).toBe("ok");
    });
    first.unmount();

    const second = renderHook(() => useCommand("cmd_c", fetcher));
    await waitFor(() => {
      expect(second.result.current.error).toMatch(/boom/);
    });
    expect(second.result.current.data).toBe("ok"); // stale-while-error
  });

  it("invalidateCommands drops the cache so the next mount loads again", async () => {
    const fetcher = vi.fn().mockResolvedValue(42);
    const first = renderHook(() => useCommand("cmd_d", fetcher));
    await waitFor(() => {
      expect(first.result.current.data).toBe(42);
    });
    first.unmount();

    act(() => {
      invalidateCommands();
    });
    const second = renderHook(() => useCommand("cmd_d", fetcher));
    expect(second.result.current.loading).toBe(true);
    expect(second.result.current.data).toBeUndefined();
    await waitFor(() => {
      expect(second.result.current.data).toBe(42);
    });
  });
});
