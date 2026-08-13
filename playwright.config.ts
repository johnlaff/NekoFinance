import { defineConfig } from "@playwright/test";

const baseURL = "http://127.0.0.1:1420";

export default defineConfig({
  expect: {
    timeout: 5_000,
    // Tolerância ABSOLUTA, nunca proporcional: em fullPage 1440×1000 uma razão de
    // 2% valeria ~57k pixels e engoliria uma frase inteira de copy sem reprovar.
    // O piso é o antialiasing entre runners distintos (centenas de pixels em
    // fullPage — a mesma máquina renderiza com diferença de unidades); o teto é o
    // menor sinal que o SCREENSHOT precisa pegar — mudança de layout, cor ou
    // espaçamento custa milhares. Copy não entra nessa conta: texto é travado
    // pelos aria snapshots ao lado de cada screenshot, sem tolerância nenhuma.
    toHaveScreenshot: {
      maxDiffPixels: 500,
    },
  },
  fullyParallel: false,
  forbidOnly: Boolean(process.env.CI),
  outputDir: "test-results/e2e-artifacts",
  projects: [
    {
      name: "chromium",
      use: {
        browserName: "chromium",
        viewport: { width: 1440, height: 960 },
        // O formato dos controles nativos (input time: 24h × 12h AM/PM) segue
        // o locale do PROCESSO do browser — env no Linux, --lang em outros
        // SOs; o `locale` do context não o alcança. Sem o pin, um runner
        // en-US alarga o input, o texto vizinho quebra uma linha a mais e o
        // screenshot diverge do baseline gerado em pt-BR.
        launchOptions: {
          args: ["--lang=pt-BR"],
          env: {
            ...(process.env as Record<string, string>),
            LANG: "pt_BR.UTF-8",
            LC_ALL: "pt_BR.UTF-8",
            LANGUAGE: "pt_BR",
          },
        },
      },
    },
  ],
  reporter: process.env.CI
    ? [
        ["line"],
        ["html", { open: "never", outputFolder: "playwright-report" }],
        ["json", { outputFile: "test-results/e2e-results.json" }],
      ]
    : [["list"], ["html", { open: "never", outputFolder: "playwright-report" }]],
  retries: process.env.CI ? 1 : 0,
  testDir: "./tests/e2e",
  timeout: 30_000,
  use: {
    baseURL,
    // Controles nativos (ex.: input time) renderizam pelo locale do browser —
    // sem pinar, um runner en-US mostra 12h com AM/PM, o input alarga e o
    // layout quebra linha diferente do baseline gerado em pt-BR.
    locale: "pt-BR",
    timezoneId: "America/Sao_Paulo",
    // O app agora abre no tema que o SO prefere quando nada foi salvo ainda
    // (prefers-color-scheme). O default do Playwright para colorScheme é "light";
    // sem pinar aqui, todo teste que não seta `neko-theme` no localStorage passaria
    // a abrir em light e divergir dos baselines "-dark" existentes. Um teste que
    // queira exercitar o default do SO faz `page.emulateMedia({ colorScheme: ... })`
    // por conta própria, sem tocar neste padrão.
    colorScheme: "dark",
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
    video: "retain-on-failure",
  },
  webServer: {
    command: "npm run dev -- --host 127.0.0.1",
    // Credenciais Google zeradas: o smoke visual precisa renderizar IGUAL com e
    // sem `.env` local (a CI não tem client id; um baseline gerado com ele
    // esconderia a linha "Conexão Google indisponível" e quebraria lá).
    env: { VITE_GOOGLE_CLIENT_ID: "", VITE_GOOGLE_DESKTOP_CLIENT_KEY: "" },
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    url: baseURL,
  },
  workers: process.env.CI ? 1 : undefined,
});
