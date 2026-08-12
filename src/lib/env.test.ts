import { afterEach, describe, expect, it, vi } from "vitest";

/**
 * `isTauri`/`isAndroid` são `const` de nível de módulo, resolvidas uma única vez no import —
 * por isso cada caso aqui ajusta `window` e força um reimport com `vi.resetModules()` em vez de
 * importar `./env` no topo do arquivo.
 */
describe("isAndroid — resolução no import do módulo", () => {
  afterEach(() => {
    // Restaura a base que src/test/setup.ts define para o resto da suíte: shell Tauri desktop,
    // com o plugin de OS já pronto.
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: {},
      configurable: true,
    });
    Object.defineProperty(window, "__TAURI_OS_PLUGIN_INTERNALS__", {
      value: { platform: "linux" },
      configurable: true,
    });
    vi.resetModules();
  });

  it("não lança e resolve false quando o shell expõe __TAURI_INTERNALS__ mas o plugin de OS ainda não injetou seu global", async () => {
    // Reproduz o defeito real: um shell/mock parcial de Tauri (o mock de e2e, antes do fix)
    // define __TAURI_INTERNALS__ sem __TAURI_OS_PLUGIN_INTERNALS__. Sem o guard extra,
    // `platform()` lança no import ("Cannot read properties of undefined") e derruba o módulo
    // inteiro — e com ele o app, já que `env.ts` é importado bem cedo na árvore.
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: {},
      configurable: true,
    });
    Reflect.deleteProperty(window, "__TAURI_OS_PLUGIN_INTERNALS__");
    vi.resetModules();

    const mod = await import("./env");

    expect(mod.isAndroid).toBe(false);
  });

  it("resolve true quando o plugin de OS está pronto e reporta android", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: {},
      configurable: true,
    });
    Object.defineProperty(window, "__TAURI_OS_PLUGIN_INTERNALS__", {
      value: { platform: "android" },
      configurable: true,
    });
    vi.resetModules();

    const mod = await import("./env");

    expect(mod.isAndroid).toBe(true);
  });

  it("resolve false num browser puro, sem nenhum global do Tauri", async () => {
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
    Reflect.deleteProperty(window, "__TAURI_OS_PLUGIN_INTERNALS__");
    vi.resetModules();

    const mod = await import("./env");

    expect(mod.isAndroid).toBe(false);
  });
});
