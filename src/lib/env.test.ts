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

describe("GOOGLE_OAUTH_CLIENT_ID — escolha de client id por plataforma", () => {
  afterEach(() => {
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

  // O Android tem que enviar o MESMO client id que emitiu o `code`/`refresh_token` — reverter
  // esta escolha para o id Desktop do env não quebra a build (os dois são strings), só o
  // consentimento real no aparelho. Sem este teste, essa reversão passa silenciosa.
  it("usa a constante da credencial Android quando isAndroid é true, nunca o client id do env", async () => {
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
    expect(mod.GOOGLE_OAUTH_CLIENT_ID).toBe(mod.GOOGLE_ANDROID_CLIENT_ID);
    expect(mod.GOOGLE_OAUTH_CLIENT_ID).not.toBe(mod.GOOGLE_CLIENT_ID);
  });

  it("usa o client id do env fora do Android", async () => {
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
    Reflect.deleteProperty(window, "__TAURI_OS_PLUGIN_INTERNALS__");
    vi.resetModules();

    const mod = await import("./env");

    expect(mod.isAndroid).toBe(false);
    expect(mod.GOOGLE_OAUTH_CLIENT_ID).toBe(mod.GOOGLE_CLIENT_ID);
  });
});
