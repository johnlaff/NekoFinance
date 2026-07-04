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

  // Regressão do swap abrupto: o tema NÃO pode trocar antes de o overlay cobrir a
  // tela (fim da animação de crescimento) — trocar cedo deixava o reveal invisível.
  it("só troca o tema quando o círculo termina de crescer e depois remove o overlay", async () => {
    const user = userEvent.setup();
    render(<ThemeToggle />);
    await user.click(screen.getByRole("button", { name: "Alternar para tema claro" }));

    // Uma única animação (grow); crescimento em andamento — tema antigo ainda vale.
    expect(anims.length).toBe(1);
    expect(document.documentElement.getAttribute("data-theme")).toBeNull();

    // Fim do crescimento → tema troca por baixo do overlay cheio.
    anims[0]!.fire("finish");
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    expect(localStorage.getItem("neko-theme")).toBe("light");
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Alternar para tema escuro" }),
      ).toBeInTheDocument(),
    );

    // Sem animação de saída: o overlay é removido no requestAnimationFrame seguinte
    // (depois que a nova paleta pintou). Nenhuma segunda animação é criada.
    await waitFor(() =>
      expect(
        document.querySelector("[aria-hidden='true'][style*='clip-path']"),
      ).toBeNull(),
    );
    expect(anims.length).toBe(1);
  });

  it("crescimento cancelado ainda troca o tema e remove o overlay", async () => {
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
