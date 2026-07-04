import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ThemeToggle } from "./ThemeToggle";

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

  // Reveal "drain": o tema troca JÁ (UI nova pinta escondida sob o overlay da cor antiga)
  // e o overlay encolhe revelando-a. Uma única animação (drain, clip full→0).
  it("troca o tema imediatamente, cobre com a cor antiga e remove ao fim do drain", async () => {
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

    // Fim do drain (clip já em 0, overlay invisível) → removido. Sem segunda animação.
    anims[0]!.fire("finish");
    expect(
      document.querySelector("[aria-hidden='true'][style*='clip-path']"),
    ).toBeNull();
    expect(anims.length).toBe(1);
  });

  it("drain cancelado remove o overlay e o tema permanece trocado", async () => {
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
