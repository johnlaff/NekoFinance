import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type * as EnvModule from "../lib/env";

/**
 * `playThemeReveal` escolhe entre dois caminhos: a View Transitions API (pseudo-elemento
 * animado) ou o overlay manual (clip-path com furo, o único confirmado visualmente nas
 * WebViews embutidas do Tauri). A escolha depende de `isTauri`, uma constante fixada na
 * carga do módulo `../lib/env` — por isso cada cenário reinicia os módulos e mocka
 * `../lib/env` ANTES de importar `./themeReveal`, em vez de tentar mutar `window` depois
 * que o módulo já carregou.
 */

interface LooseDoc {
  startViewTransition?: unknown;
}

function stubWaapiOnRoot(): ReturnType<typeof vi.fn> {
  const spy = vi.fn();
  (document.documentElement as unknown as { animate: unknown }).animate = spy;
  return spy;
}

function stubWaapiOnElements(): void {
  (HTMLElement.prototype as unknown as { animate: unknown }).animate = vi.fn(() => ({
    addEventListener: vi.fn(),
  }));
}

beforeEach(() => {
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
  vi.resetModules();
});

afterEach(() => {
  delete (document as unknown as LooseDoc).startViewTransition;
  delete (document.documentElement as unknown as { animate?: unknown }).animate;
  delete (HTMLElement.prototype as unknown as { animate?: unknown }).animate;
  document
    .querySelectorAll("[aria-hidden='true'][style*='clip-path']")
    .forEach((el) => el.remove());
  vi.restoreAllMocks();
  vi.doUnmock("../lib/env");
});

describe("playThemeReveal — dentro do shell do Tauri (isTauri=true)", () => {
  it("ignora a View Transitions API mesmo presente e usa o overlay confirmado", async () => {
    vi.doMock("../lib/env", async (importOriginal) => {
      const actual = await importOriginal<typeof EnvModule>();
      return { ...actual, isTauri: true };
    });
    (document as unknown as LooseDoc).startViewTransition = (cb: () => void) => {
      cb();
      return { ready: Promise.resolve() };
    };
    const rootAnimateSpy = stubWaapiOnRoot();
    stubWaapiOnElements();

    const { playThemeReveal } = await import("./themeReveal");
    const apply = vi.fn();
    playThemeReveal(10, 20, 100, "light", apply);

    // Tema trocado de imediato pelo caminho manual — não dentro do callback assíncrono
    // de startViewTransition — e o overlay de cobertura entra no DOM.
    expect(apply).toHaveBeenCalledTimes(1);
    expect(
      document.querySelector("[aria-hidden='true'][style*='clip-path']"),
    ).not.toBeNull();
    expect(rootAnimateSpy).not.toHaveBeenCalled();
  });
});

describe("playThemeReveal — fora do Tauri, num browser comum (isTauri=false)", () => {
  it("usa a View Transitions API quando presente: troca no callback e anima o pseudo-elemento", async () => {
    vi.doMock("../lib/env", async (importOriginal) => {
      const actual = await importOriginal<typeof EnvModule>();
      return { ...actual, isTauri: false };
    });
    (document as unknown as LooseDoc).startViewTransition = (cb: () => void) => {
      cb();
      return { ready: Promise.resolve() };
    };
    const rootAnimateSpy = stubWaapiOnRoot();

    const { playThemeReveal } = await import("./themeReveal");
    const apply = vi.fn();
    playThemeReveal(10, 20, 100, "light", apply);

    // Tema trocado dentro do callback da transição; nenhum overlay de cobertura criado.
    expect(apply).toHaveBeenCalledTimes(1);
    expect(
      document.querySelector("[aria-hidden='true'][style*='clip-path']"),
    ).toBeNull();

    await vi.waitFor(() => expect(rootAnimateSpy).toHaveBeenCalledTimes(1));
    const opts = rootAnimateSpy.mock.calls[0]![1] as {
      pseudoElement?: string;
      duration?: number;
    };
    expect(opts.pseudoElement).toBe("::view-transition-new(root)");
    expect(opts.duration).toBeGreaterThan(100); // constante, não o token "~0"
  });

  it("sem a API, usa o overlay manual (mesmo caminho do Tauri)", async () => {
    vi.doMock("../lib/env", async (importOriginal) => {
      const actual = await importOriginal<typeof EnvModule>();
      return { ...actual, isTauri: false };
    });
    stubWaapiOnElements();

    const { playThemeReveal } = await import("./themeReveal");
    const apply = vi.fn();
    playThemeReveal(10, 20, 100, "dark", apply);

    expect(apply).toHaveBeenCalledTimes(1);
    expect(
      document.querySelector("[aria-hidden='true'][style*='clip-path']"),
    ).not.toBeNull();
  });
});
