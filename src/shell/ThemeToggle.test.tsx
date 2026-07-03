import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ThemeToggle } from "./ThemeToggle";

describe("ThemeToggle", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
  });

  it("switches themes via the fallback path when View Transitions are unavailable", async () => {
    const user = userEvent.setup();
    render(<ThemeToggle />);

    // jsdom has no document.startViewTransition → instant swap path.
    await user.click(screen.getByRole("button", { name: "Alternar para tema claro" }));
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    expect(localStorage.getItem("neko-theme")).toBe("light");

    await user.click(screen.getByRole("button", { name: "Alternar para tema escuro" }));
    expect(document.documentElement.getAttribute("data-theme")).toBeNull();
    expect(localStorage.getItem("neko-theme")).toBe("dark");
  });
});

describe("ThemeToggle — reveal via View Transitions", () => {
  // jsdom não tem a API; o mock cobre só o que o componente usa (ready), com cast
  // frouxo porque o ViewTransition da lib DOM tem mais membros que o necessário.
  interface LooseDoc {
    startViewTransition?: unknown;
  }
  const setVT = (fn: (cb: () => void) => unknown) => {
    (document as unknown as LooseDoc).startViewTransition = fn;
  };

  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
  });

  afterEach(() => {
    delete (document as unknown as LooseDoc).startViewTransition;
  });

  // Regressão da race que deixava o reveal invisível: o tema só pode ser aplicado
  // DENTRO do callback da transição (depois do snapshot do tema velho). Um setTheme
  // síncrono no clique dispararia o effect que aplica o tema antes da captura.
  it("aplica o tema somente dentro do callback da transição, nunca antes", async () => {
    let themeAtCallbackStart: string | null = "sentinela";
    const snapshot = Promise.withResolvers<void>();

    setVT((cb) => {
      // Captura o estado do DOM no momento do "snapshot" (antes do callback).
      themeAtCallbackStart = document.documentElement.getAttribute("data-theme");
      cb();
      snapshot.resolve();
      return { ready: Promise.resolve(), finished: Promise.resolve() };
    });

    const user = userEvent.setup();
    render(<ThemeToggle />);
    await user.click(screen.getByRole("button", { name: "Alternar para tema claro" }));
    await snapshot.promise;

    // No momento do snapshot o tema velho (dark = sem atributo) ainda valia.
    expect(themeAtCallbackStart).toBeNull();
    // Depois do callback, o DOM tem o tema novo e o estado React acompanhou.
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Alternar para tema escuro" }),
      ).toBeInTheDocument(),
    );
  });

  it("transição abortada ainda sincroniza o estado (ready rejeita)", async () => {
    setVT((cb) => {
      cb();
      return {
        ready: Promise.reject(new Error("skipped")),
        finished: Promise.resolve(),
      };
    });
    const user = userEvent.setup();
    render(<ThemeToggle />);
    await user.click(screen.getByRole("button", { name: "Alternar para tema claro" }));

    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Alternar para tema escuro" }),
      ).toBeInTheDocument(),
    );
  });

  it("startViewTransition quebrado em runtime cai para o overlay sem perder a troca", async () => {
    setVT(
      vi.fn(() => {
        throw new Error("boom");
      }),
    );
    const user = userEvent.setup();
    render(<ThemeToggle />);
    await user.click(screen.getByRole("button", { name: "Alternar para tema claro" }));

    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
  });
});
