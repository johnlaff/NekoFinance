import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  COMMAND_MAX_ATTEMPTS,
  COMMAND_TIMEOUT_MS,
  invalidateCommands,
  useCommand,
} from "./useCommand";

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
      expect(second.result.current.error).toMatch(/Não foi possível concluir/);
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

  it("invalidateCommands refetches hooks that continue montados (retry/pós-escrita)", async () => {
    // Sem isto, "Tentar novamente" e o refresh pós-escrita em tela já montada eram no-op:
    // o cache era limpo mas o effect só re-rodava no próximo mount.
    const fetcher = vi.fn().mockResolvedValueOnce("v1").mockResolvedValueOnce("v2");
    const { result } = renderHook(() => useCommand("cmd_refetch", fetcher));
    await waitFor(() => {
      expect(result.current.data).toBe("v1");
    });

    act(() => {
      invalidateCommands();
    });
    await waitFor(() => {
      expect(result.current.data).toBe("v2");
    });
    expect(fetcher).toHaveBeenCalledTimes(2);
  });

  it("does not keep previous command data visible after the key changes", async () => {
    const fetchA = vi.fn().mockResolvedValue("june");
    let resolveFetchB!: (value: string) => void;
    const fetchB = vi.fn(
      () =>
        new Promise<string>((resolve) => {
          resolveFetchB = resolve;
        }),
    );
    const { result, rerender } = renderHook(
      ({ cmd, fetcher }: { cmd: string; fetcher: () => Promise<string> }) =>
        useCommand(cmd, fetcher),
      { initialProps: { cmd: "month:2026-06", fetcher: fetchA } },
    );
    await waitFor(() => {
      expect(result.current.data).toBe("june");
    });

    rerender({ cmd: "month:2026-07", fetcher: fetchB });

    await waitFor(() => {
      expect(result.current.data).toBeUndefined();
      expect(result.current.loading).toBe(true);
    });

    act(() => {
      resolveFetchB("july");
    });
    await waitFor(() => {
      expect(result.current.data).toBe("july");
    });
  });

  // Regressão #486: no cold start Android o primeiro `invoke` às vezes não assenta —
  // nem resolve nem rejeita, sem log dos dois lados (a corrida do bridge de IPC não
  // pronto). A tela Hoje ficava presa no esqueleto pra sempre; só trocar de aba e
  // voltar (uma NOVA montagem, um NOVO invoke) resolvia. O teto por tentativa cobre
  // exatamente essa promessa que nunca assenta, sem depender do usuário remontar a tela.
  it("retries on its own when a command never settles, without needing a remount", async () => {
    vi.useFakeTimers();
    try {
      let calls = 0;
      const fetcher = vi.fn(() => {
        calls += 1;
        if (calls === 1) return new Promise<number>(() => undefined); // nunca assenta
        return Promise.resolve(99);
      });
      const { result } = renderHook(() => useCommand("cmd_stuck", fetcher));
      expect(result.current.loading).toBe(true);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(COMMAND_TIMEOUT_MS + 50);
      });

      expect(result.current.data).toBe(99);
      expect(result.current.loading).toBe(false);
      expect(result.current.error).toBeNull();
      expect(fetcher).toHaveBeenCalledTimes(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it("gives up after exhausting retries and surfaces an error instead of spinning forever", async () => {
    vi.useFakeTimers();
    try {
      const fetcher = vi.fn(() => new Promise<number>(() => undefined)); // nunca assenta, sempre
      const { result } = renderHook(() => useCommand("cmd_dead", fetcher));

      await act(async () => {
        await vi.advanceTimersByTimeAsync(
          COMMAND_TIMEOUT_MS * COMMAND_MAX_ATTEMPTS + 200,
        );
      });

      expect(result.current.loading).toBe(false);
      expect(result.current.data).toBeUndefined();
      expect(result.current.error).toMatch(/conexão falhou/i);
      expect(fetcher).toHaveBeenCalledTimes(COMMAND_MAX_ATTEMPTS);
    } finally {
      vi.useRealTimers();
    }
  });
});
