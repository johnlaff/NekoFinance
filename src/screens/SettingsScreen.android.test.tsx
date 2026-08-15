import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { SettingsScreen } from "./SettingsScreen";
import { NekoAppProvider } from "../shell/appContext";
import { APP_INFO, mockCommands, mockInvoke } from "../test/commands";
import type * as ConfigView from "./configView";
import {
  fetchMiaConsent,
  grantMiaConsentCmd,
  revokeMiaConsentCmd,
  setMiaApiKeyCmd,
  type MiaConsentView,
} from "./configView";

vi.mock("../lib/env", async (importOriginal) => ({
  ...(await importOriginal()),
  isAndroid: true,
  GOOGLE_CLIENT_ID: "test-client-id.apps.googleusercontent.com",
  GOOGLE_OAUTH_CLIENT_ID: "test-android-client-id.apps.googleusercontent.com",
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-fs", () => ({
  readFile: vi.fn(),
  writeFile: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({
  appCacheDir: vi.fn().mockResolvedValue("/cache"),
  join: vi.fn().mockResolvedValue("/cache/neko-local-import.xlsx"),
}));

vi.mock("./configView", async (importOriginal) => {
  const actual = await importOriginal<typeof ConfigView>();
  return {
    ...actual,
    fetchMiaConsent: vi.fn(),
    grantMiaConsentCmd: vi.fn(),
    revokeMiaConsentCmd: vi.fn(),
    setMiaApiKeyCmd: vi.fn(),
  };
});

function consent(overrides: Partial<MiaConsentView> = {}): MiaConsentView {
  return {
    granted: false,
    needs_renewal: false,
    granted_at: null,
    has_key: false,
    linked: false,
    text: {
      headline: "Autorizar a conversa aberta",
      processors: [],
      paragraphs: [],
      checklist: [],
    },
    ...overrides,
  };
}

const appCtx = { navigate: vi.fn(), openCompose: vi.fn() };

describe("SettingsScreen — Android (isAndroid: true)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    (fetchMiaConsent as ReturnType<typeof vi.fn>)
      .mockReset()
      .mockResolvedValue(consent());
    (grantMiaConsentCmd as ReturnType<typeof vi.fn>).mockReset();
    (revokeMiaConsentCmd as ReturnType<typeof vi.fn>).mockReset();
    (setMiaApiKeyCmd as ReturnType<typeof vi.fn>).mockReset();
  });

  it("identifica a plataforma como Tauri Android no rodapé, nunca desktop", async () => {
    mockCommands({ get_app_info: APP_INFO });
    render(
      <NekoAppProvider value={appCtx}>
        <SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />
      </NekoAppProvider>,
    );

    expect(await screen.findByText(/Tauri Android/)).toBeInTheDocument();
    expect(screen.queryByText(/Tauri desktop/)).not.toBeInTheDocument();
  });

  describe("mitigação do segundo deep link (upstream tauri-apps/plugins-workspace#2397)", () => {
    it("avisa a partir da primeira reconexão nesta sessão, nunca antes dela", async () => {
      const user = userEvent.setup();
      mockCommands({
        get_app_info: APP_INFO,
        start_oauth_flow: "oauth_started",
        check_auth_status: "connected",
      });
      render(
        <NekoAppProvider value={appCtx}>
          <SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />
        </NekoAppProvider>,
      );

      const noteText = /pode não retornar ao app sozinha/;
      expect(screen.queryByText(noteText)).not.toBeInTheDocument();

      // O aviso nasce da própria contagem de cliques (síncrona ao handler), não do desfecho do
      // fluxo OAuth em segundo plano — não precisa esperar o poll (2s) resolver.
      await user.click(screen.getByRole("button", { name: "Reconectar" }));
      expect(screen.getByText(noteText)).toBeInTheDocument();
    });
  });
});
