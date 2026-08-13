import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ThemeToggle } from "./ThemeToggle";

/** Mock mínimo de matchMedia, só para a query de prefers-color-scheme. */
function mockPrefersColorScheme(prefersLight: boolean) {
  vi.stubGlobal(
    "matchMedia",
    vi.fn((query: string) => ({
      matches: query === "(prefers-color-scheme: light)" && prefersLight,
      media: query,
    })),
  );
}

describe("ThemeToggle", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
  });

  it("switches themes instantly when WAAPI is unavailable (jsdom path)", async () => {
    const user = userEvent.setup();
    render(<ThemeToggle />);

    // jsdom has no Element.animate → o guard troca o tema sem floreio.
    await user.click(screen.getByRole("button", { name: "Alternar para tema claro" }));
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    expect(localStorage.getItem("neko-theme")).toBe("light");

    await user.click(screen.getByRole("button", { name: "Alternar para tema escuro" }));
    expect(document.documentElement.getAttribute("data-theme")).toBeNull();
    expect(localStorage.getItem("neko-theme")).toBe("dark");
  });
});

describe("ThemeToggle — reveal por overlay (WAAPI em elemento real)", () => {
  interface FakeAnim {
    listeners: Record<string, (() => void)[]>;
    addEventListener: (type: string, cb: () => void) => void;
    fire: (type: string) => void;
  }

  const mkFakeAnim = (): FakeAnim => {
    const listeners: Record<string, (() => void)[]> = {};
    return {
      listeners,
      addEventListener: (type, cb) => {
        (listeners[type] ??= []).push(cb);
      },
      fire: (type) => {
        for (const cb of listeners[type] ?? []) cb();
      },
    };
  };

  let anims: FakeAnim[] = [];

  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
    anims = [];
    // jsdom não tem WAAPI; injeta um fake que captura os listeners de cada animate().
    (HTMLElement.prototype as unknown as { animate: unknown }).animate = vi.fn(() => {
      const anim = mkFakeAnim();
      anims.push(anim);
      return anim;
    });
  });

  afterEach(() => {
    delete (HTMLElement.prototype as unknown as { animate?: unknown }).animate;
  });

  // Reveal "buraco crescente": o tema troca JÁ (UI nova pinta escondida sob o overlay da
  // cor antiga) e um furo circular cresce revelando-a. Uma única animação (o clip).
  it("troca o tema imediatamente, cobre com a cor antiga e remove ao fim do reveal", async () => {
    const user = userEvent.setup();
    render(<ThemeToggle />);
    await user.click(screen.getByRole("button", { name: "Alternar para tema claro" }));

    // Tema já trocado no clique; overlay presente; uma animação (o drain).
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    expect(localStorage.getItem("neko-theme")).toBe("light");
    expect(anims.length).toBe(1);
    expect(
      document.querySelector("[aria-hidden='true'][style*='clip-path']"),
    ).not.toBeNull();
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Alternar para tema escuro" }),
      ).toBeInTheDocument(),
    );

    // Fim do reveal (furo cheio, overlay invisível) → removido no frame seguinte.
    anims[0]!.fire("finish");
    await waitFor(() =>
      expect(
        document.querySelector("[aria-hidden='true'][style*='clip-path']"),
      ).toBeNull(),
    );
    expect(anims.length).toBe(1);
  });

  it("reveal cancelado remove o overlay e o tema permanece trocado", async () => {
    const user = userEvent.setup();
    render(<ThemeToggle />);
    await user.click(screen.getByRole("button", { name: "Alternar para tema claro" }));

    anims[0]!.fire("cancel");
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    expect(
      document.querySelector("[aria-hidden='true'][style*='clip-path']"),
    ).toBeNull();
  });
});

describe("ThemeToggle — default do sistema (prefers-color-scheme)", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("abre em light quando o SO prefere light e nada foi salvo ainda", async () => {
    mockPrefersColorScheme(true);
    render(<ThemeToggle />);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Alternar para tema escuro" }),
      ).toBeInTheDocument(),
    );
  });

  it("abre em dark quando o SO prefere dark e nada foi salvo ainda", async () => {
    mockPrefersColorScheme(false);
    render(<ThemeToggle />);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Alternar para tema claro" }),
      ).toBeInTheDocument(),
    );
  });

  it("a escolha explícita salva vence o prefers-color-scheme atual", async () => {
    localStorage.setItem("neko-theme", "dark");
    mockPrefersColorScheme(true);
    render(<ThemeToggle />);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Alternar para tema claro" }),
      ).toBeInTheDocument(),
    );
  });
});

describe("ThemeToggle — caminho View Transitions (quando disponível)", () => {
  interface LooseDoc {
    startViewTransition?: unknown;
  }
  let rootAnimateSpy: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
    // Mock da VT: roda o callback (swap) e resolve ready; captura a animação do pseudo.
    (document as unknown as LooseDoc).startViewTransition = (cb: () => void) => {
      cb();
      return { ready: Promise.resolve() };
    };
    rootAnimateSpy = vi.fn();
    (document.documentElement as unknown as { animate: unknown }).animate =
      rootAnimateSpy;
  });

  afterEach(() => {
    delete (document as unknown as LooseDoc).startViewTransition;
    delete (document.documentElement as unknown as { animate?: unknown }).animate;
  });

  it("usa a View Transitions API: troca o tema no callback e anima o pseudo-elemento", async () => {
    const user = userEvent.setup();
    render(<ThemeToggle />);
    await user.click(screen.getByRole("button", { name: "Alternar para tema claro" }));

    // Tema trocado (dentro do callback da transição) e nenhum overlay de cobertura criado.
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    expect(
      document.querySelector("[aria-hidden='true'][style*='clip-path']"),
    ).toBeNull();

    // Após ready: anima o clip-path no ::view-transition-new(root) com duração fixa.
    await waitFor(() => expect(rootAnimateSpy).toHaveBeenCalledTimes(1));
    const opts = rootAnimateSpy.mock.calls[0]![1] as {
      pseudoElement?: string;
      duration?: number;
    };
    expect(opts.pseudoElement).toBe("::view-transition-new(root)");
    expect(opts.duration).toBeGreaterThan(100); // constante, não o token "~0"
  });
});
