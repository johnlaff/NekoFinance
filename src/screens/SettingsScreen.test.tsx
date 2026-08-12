import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { SettingsScreen } from "./SettingsScreen";
import { NekoAppProvider } from "../shell/appContext";
import { APP_INFO, POCKETS, mockCommands, mockInvoke } from "../test/commands";
import { invalidateCommands } from "../lib/useCommand";
import { open } from "@tauri-apps/plugin-dialog";
import type * as ConfigView from "./configView";
import type * as Env from "../lib/env";
import {
  fetchMiaConsent,
  grantMiaConsentCmd,
  revokeMiaConsentCmd,
  setMiaApiKeyCmd,
  type MiaConsentView,
} from "./configView";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

vi.mock("../lib/env", async (importOriginal) => ({
  ...(await importOriginal<typeof Env>()),
  GOOGLE_CLIENT_ID: "test-client-id.apps.googleusercontent.com",
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

const mockOpen = open as ReturnType<typeof vi.fn>;
const mockGetMiaConsent = fetchMiaConsent as ReturnType<typeof vi.fn>;
const mockGrantMiaConsent = grantMiaConsentCmd as ReturnType<typeof vi.fn>;
const mockRevokeMiaConsent = revokeMiaConsentCmd as ReturnType<typeof vi.fn>;
const mockSetMiaApiKey = setMiaApiKeyCmd as ReturnType<typeof vi.fn>;

const CONSENT_TEXT = {
  headline: "Autorizar a conversa aberta",
  processors: [
    { name: "OpenRouter", role: "Roteia o pedido ao modelo escolhido." },
    { name: "Amazon Bedrock", role: "Executa o modelo para responder." },
  ],
  paragraphs: [
    "Suas perguntas podem sair deste aparelho.",
    "A conversa usa sua chave.",
  ],
  checklist: [
    {
      title: "Desligue o treino com o que você envia",
      detail:
        "Na sua conta do provedor, recuse provedores que treinam com as suas entradas.",
    },
    {
      title: "Desligue a publicação de prompts em endpoints gratuitos",
      detail: "Essa escolha também vive só na sua conta do provedor.",
    },
  ],
};

function consent(overrides: Partial<MiaConsentView> = {}): MiaConsentView {
  return {
    granted: false,
    needs_renewal: false,
    granted_at: null,
    has_key: false,
    linked: false,
    text: CONSENT_TEXT,
    ...overrides,
  };
}

const appCtx = { navigate: vi.fn(), openCompose: vi.fn() };
function renderSettings() {
  return render(
    <NekoAppProvider value={appCtx}>
      <SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />
    </NekoAppProvider>,
  );
}

describe("SettingsScreen", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockOpen.mockReset();
    mockGetMiaConsent.mockReset();
    mockGrantMiaConsent.mockReset();
    mockRevokeMiaConsent.mockReset();
    mockSetMiaApiKey.mockReset();
    mockGetMiaConsent.mockResolvedValue(consent());
  });

  it("mostra a conversa desligada e revela os processadores e opt-ins ao ligar", async () => {
    const user = userEvent.setup();
    mockCommands({ get_app_info: APP_INFO });
    renderSettings();

    expect(
      await screen.findByText(
        "Sem autorização — a Mia responde só o que ela calcula aqui dentro.",
      ),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Autorizar" }));

    expect(screen.getByText("OpenRouter")).toBeInTheDocument();
    expect(screen.getByText("Amazon Bedrock")).toBeInTheDocument();
    expect(
      screen.getByText("Desligue o treino com o que você envia"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Desligue a publicação de prompts em endpoints gratuitos"),
    ).toBeInTheDocument();
  });

  it("guarda a chave antes de registrar e passa a mostrar a conversa ligada", async () => {
    const user = userEvent.setup();
    mockCommands({ get_app_info: APP_INFO });
    mockSetMiaApiKey.mockResolvedValue(consent({ has_key: true }));
    mockGrantMiaConsent.mockResolvedValue(
      consent({ granted: true, has_key: true, linked: true }),
    );
    renderSettings();

    await user.click(await screen.findByRole("button", { name: "Autorizar" }));
    await user.type(screen.getByLabelText("Sua chave do provedor"), "chave-de-teste");
    await user.click(screen.getByRole("button", { name: "Registrar consentimento" }));

    await waitFor(() => {
      expect(mockSetMiaApiKey).toHaveBeenCalledWith("chave-de-teste");
      expect(mockGrantMiaConsent).toHaveBeenCalledOnce();
    });
    expect(
      screen.getByText("Autorizada · OpenRouter e Amazon Bedrock"),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "Conversa aberta autorizada — suas perguntas e os lançamentos necessários podem ir para OpenRouter e Amazon Bedrock.",
      ),
    ).toBeInTheDocument();
    expect(screen.getAllByText("Nuvem")).toHaveLength(1);
  });

  it("revoga o consentimento e devolve a conversa ao estado desligado", async () => {
    const user = userEvent.setup();
    mockCommands({ get_app_info: APP_INFO });
    mockGetMiaConsent.mockResolvedValue(
      consent({ granted: true, has_key: true, linked: true }),
    );
    mockRevokeMiaConsent.mockResolvedValue(consent());
    renderSettings();

    await user.click(await screen.findByRole("button", { name: "Revogar" }));
    await user.click(screen.getByRole("button", { name: "Revogar e apagar a chave" }));

    await waitFor(() => expect(mockRevokeMiaConsent).toHaveBeenCalledOnce());
    expect(
      screen.getByText(
        "Sem autorização — a Mia responde só o que ela calcula aqui dentro.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Responde local. Nada sai deste aparelho."),
    ).toBeInTheDocument();
    expect(screen.getAllByText("Local")).toHaveLength(3);
  });

  it("pede para rever quando a versão do texto exige renovação", async () => {
    mockCommands({ get_app_info: APP_INFO });
    mockGetMiaConsent.mockResolvedValue(
      consent({ needs_renewal: true, has_key: true }),
    );
    renderSettings();

    expect(
      await screen.findByText("O texto mudou — leia de novo para seguir autorizada."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Rever" })).toBeInTheDocument();
  });

  it("nunca mostra o valor de uma chave guardada", async () => {
    const user = userEvent.setup();
    mockCommands({ get_app_info: APP_INFO });
    mockGetMiaConsent.mockResolvedValue(consent({ has_key: true }));
    renderSettings();

    await user.click(await screen.findByRole("button", { name: "Continuar" }));
    expect(screen.getByText("Chave guardada")).toBeInTheDocument();
    expect(screen.queryByLabelText("Sua chave do provedor")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Trocar" }));
    expect(screen.getByLabelText("Sua chave do provedor")).toHaveValue("");
  });

  it("troca a paleta de acento pelo seletor e persiste no :root", async () => {
    const user = userEvent.setup();
    mockCommands({ get_app_info: APP_INFO });
    localStorage.removeItem("neko-accent");
    document.documentElement.removeAttribute("data-accent");
    renderSettings();

    const group = screen.getByRole("radiogroup", { name: "Cor de destaque" });
    const jade = within(group).getByRole("radio", { name: "Jade" });
    const lima = within(group).getByRole("radio", { name: "Lima" });
    expect(jade).toHaveAttribute("aria-checked", "true");

    await user.click(lima);
    expect(document.documentElement.getAttribute("data-accent")).toBe("lima");
    expect(localStorage.getItem("neko-accent")).toBe("lima");
    expect(lima).toHaveAttribute("aria-checked", "true");
    expect(jade).toHaveAttribute("aria-checked", "false");

    await user.click(jade);
    expect(document.documentElement.hasAttribute("data-accent")).toBe(false);
    expect(localStorage.getItem("neko-accent")).toBe("jade");
  });

  it("swatches de acento: roving tabindex com setas (uma parada de Tab)", async () => {
    const user = userEvent.setup();
    mockCommands({ get_app_info: APP_INFO });
    localStorage.removeItem("neko-accent");
    document.documentElement.removeAttribute("data-accent");
    renderSettings();

    const group = screen.getByRole("radiogroup", { name: "Cor de destaque" });
    const jade = within(group).getByRole("radio", { name: "Jade" });
    const lima = within(group).getByRole("radio", { name: "Lima" });
    // Só o selecionado é parada de Tab.
    expect(jade).toHaveAttribute("tabindex", "0");
    expect(lima).toHaveAttribute("tabindex", "-1");

    // Seta seleciona o próximo e move o roving.
    jade.focus();
    await user.keyboard("{ArrowRight}");
    expect(lima).toHaveAttribute("aria-checked", "true");
    expect(lima).toHaveAttribute("tabindex", "0");
    expect(jade).toHaveAttribute("tabindex", "-1");
    expect(document.documentElement.getAttribute("data-accent")).toBe("lima");

    // Seta para trás volta ao jade (com wrap coberto pelo módulo).
    await user.keyboard("{ArrowLeft}");
    expect(jade).toHaveAttribute("aria-checked", "true");
    expect(localStorage.getItem("neko-accent")).toBe("jade");
  });

  it("diagnóstico de animações fica atrás de porta (jargão fora da leitura padrão)", async () => {
    const user = userEvent.setup();
    mockCommands({ get_app_info: APP_INFO });
    renderSettings();

    // Fechada por padrão: a região existe, mas inerte; o rótulo didático fica visível.
    expect(screen.getByText("Diagnóstico de animações")).toBeInTheDocument();
    const door = document.getElementById("config-motion-diag");
    expect(door).toHaveAttribute("inert");

    const toggle = screen.getByRole("button", { name: "Abrir" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(door).not.toHaveAttribute("inert");
    // O verdict/facts vive numa região aria-live para anunciar o resultado do teste.
    expect(door?.querySelector('[aria-live="polite"]')).not.toBeNull();
  });

  it("shows the local data location and version", async () => {
    mockCommands({ get_app_info: APP_INFO });
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText(APP_INFO.db_path)).toBeInTheDocument();
    });
    expect(screen.getByText(/v0\.1\.0/)).toBeInTheDocument();
    expect(screen.getByText(/nada de uso é enviado/)).toBeInTheDocument();
  });

  it("greet: veredito com má notícia quando desconectado", () => {
    mockCommands({ get_app_info: APP_INFO });
    renderSettings(); // authStatus="disconnected"

    expect(
      screen.getByRole("heading", { level: 1, name: "Tudo neste dispositivo" }),
    ).toBeInTheDocument();
    // "Desconectado" também aparece no sub da linha Google Sheets — mira a pílula.
    expect(
      screen.getByText("Desconectado", { selector: ".config__state b" }),
    ).toBeInTheDocument();
  });

  it("escrita só com aprovação é fato (pílula), nunca um toggle", () => {
    mockCommands({ get_app_info: APP_INFO });
    renderSettings();

    expect(screen.getByText("Escrita só com aprovação")).toBeInTheDocument();
    expect(screen.getByText("Sempre")).toBeInTheDocument();
    expect(screen.queryByRole("switch", { name: /escrita/i })).not.toBeInTheDocument();
  });

  it("porta Gerenciar abre e fecha o painel denso da conexão", async () => {
    const user = userEvent.setup();
    mockCommands({ get_app_info: APP_INFO });
    renderSettings();

    const door = screen.getByRole("button", { name: "Gerenciar" });
    expect(door).toHaveAttribute("aria-expanded", "false");
    // Fechada, a região fica inerte — nada dentro dela é focável.
    expect(document.getElementById("config-manage")).toHaveAttribute("inert");

    await user.click(door);
    expect(door).toHaveAttribute("aria-expanded", "true");
    expect(document.getElementById("config-manage")).not.toHaveAttribute("inert");
  });

  it("tema escuro é um switch que reflete o tema atual", () => {
    mockCommands({ get_app_info: APP_INFO });
    renderSettings();

    expect(screen.getByRole("switch", { name: "Tema escuro" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  it("imports a local xlsx through the native dialog and reports the result", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_app_info: APP_INFO,
      import_local_xlsx: {
        count: 12,
        summary: "Imported 12 total rows from: 2026 (12 rows)",
        diagnostics: [],
      },
    });
    mockOpen.mockResolvedValue("/home/user/financas.xlsx");

    renderSettings();

    await user.click(screen.getByRole("button", { name: /Escolher arquivo/ }));

    await waitFor(() => {
      expect(screen.getByText(/Imported 12 total rows/)).toBeInTheDocument();
    });
    expect(mockOpen).toHaveBeenCalledWith(
      expect.objectContaining({
        filters: [{ name: "Planilha", extensions: ["xlsx"] }],
      }),
    );
  });

  it("stays quiet when the dialog is dismissed", async () => {
    const user = userEvent.setup();
    mockCommands({ get_app_info: APP_INFO });
    mockOpen.mockResolvedValue(null);

    renderSettings();
    await user.click(screen.getByRole("button", { name: /Escolher arquivo/ }));

    expect(screen.queryByText(/Imported/)).not.toBeInTheDocument();
  });

  it("surfaces import errors", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_app_info: APP_INFO,
      import_local_xlsx: new Error("open error: corrupt file"),
    });
    mockOpen.mockResolvedValue("/home/user/financas.xlsx");

    renderSettings();
    await user.click(screen.getByRole("button", { name: /Escolher arquivo/ }));

    await waitFor(() => {
      expect(
        screen.getByText(/Não foi possível importar o arquivo local/),
      ).toBeInTheDocument();
    });
  });

  it("lists pockets with PT-BR type labels (spec 007)", async () => {
    mockCommands({ get_app_info: APP_INFO, get_pockets: POCKETS });
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("Vale refeição")).toBeInTheDocument();
    });
    expect(
      screen.getByText(/Vale alimentação\/refeição · Restrito/),
    ).toBeInTheDocument();
    expect(screen.getByText(/Previdência privada · Ilíquido/)).toBeInTheDocument();
  });

  it("creates a pocket with the balance parsed to cents (spec 007)", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_app_info: APP_INFO,
      get_pockets: POCKETS,
      create_account: "new-id",
    });
    renderSettings();

    await user.type(screen.getByLabelText("Nome"), "Vale alimentação");
    await user.selectOptions(screen.getByLabelText("Tipo"), "meal_voucher");
    await user.type(screen.getByLabelText("Saldo (R$)"), "1.234,56");
    await user.click(screen.getByRole("button", { name: /Adicionar bolso/ }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("create_account", {
        name: "Vale alimentação",
        accountType: "meal_voucher",
        balanceCents: 123456,
        institution: null,
      });
    });
  });

  it("rejects an unparseable balance before calling the backend (spec 007)", async () => {
    const user = userEvent.setup();
    mockCommands({ get_app_info: APP_INFO, get_pockets: POCKETS });
    renderSettings();

    await user.type(screen.getByLabelText("Nome"), "Conta");
    await user.type(screen.getByLabelText("Saldo (R$)"), "abc");
    await user.click(screen.getByRole("button", { name: /Adicionar bolso/ }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/Saldo inválido/);
    expect(mockInvoke).not.toHaveBeenCalledWith("create_account", expect.anything());
  });

  it("offers the Google connect flow when disconnected", async () => {
    mockCommands({ get_app_info: APP_INFO });
    renderSettings();
    expect(
      await screen.findByRole("button", { name: /Conectar Google/ }),
    ).toBeInTheDocument();
  });

  it("TetoLinkSection: sem teto estipulado, resume e leva à tela do teto", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_app_info: APP_INFO,
      get_daily_budget_cmd: { per_day_cents: 0, divisor_days: null, categories: [] },
    });
    renderSettings();

    expect(await screen.findByText("Sem teto estipulado.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Abrir teto do diário" }));
    expect(appCtx.navigate).toHaveBeenCalledWith("teto");
  });
  it("TetoLinkSection: com teto ativo, mostra o valor por dia", async () => {
    mockCommands({
      get_app_info: APP_INFO,
      get_daily_budget_cmd: { per_day_cents: 4033, divisor_days: 31, categories: [] },
    });
    renderSettings();

    // O valor vive num <Money> (nós separados): casamos pelo textContent da linha.
    await waitFor(() => {
      expect(
        screen.getByText(
          (_, el) =>
            el?.className === "config__what-s" &&
            /Teto estipulado: R\$\s?40,33 por dia\./.test(
              (el.textContent ?? "").replace(/\s+/g, " "),
            ),
        ),
      ).toBeInTheDocument();
    });
  });
});

describe("DailyReminderSection", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockOpen.mockReset();
  });

  // `get_app_setting` é chamado com `key`s diferentes (enabled/time), então roteamos
  // por (cmd, args) em vez do `mockCommands` que só distingue por nome de comando.
  function mockSettings(values: Record<string, string | null>) {
    invalidateCommands();
    mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "get_app_info") return Promise.resolve(APP_INFO);
      if (cmd === "get_app_setting") {
        const key = String(args?.["key"]);
        return Promise.resolve(key in values ? values[key] : null);
      }
      if (cmd === "set_app_setting") return Promise.resolve(undefined);
      // Agendamento no nível do sistema: melhor-esforço; aqui sempre resolve.
      if (cmd === "register_os_reminder" || cmd === "unregister_os_reminder")
        return Promise.resolve(undefined);
      return Promise.reject(new Error(`unmocked command: ${cmd}`));
    });
  }

  it("shows the reminder toggle in the default ON state when the key is absent", async () => {
    mockSettings({}); // chaves ausentes → ligado por padrão
    renderSettings();

    const toggle = await screen.findByRole("switch", { name: "Lembrete diário" });
    expect(toggle).toHaveAttribute("aria-checked", "true");
  });

  it("persists the toggle off", async () => {
    const user = userEvent.setup();
    mockSettings({});
    renderSettings();

    const toggle = await screen.findByRole("switch", { name: "Lembrete diário" });
    await user.click(toggle);

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("set_app_setting", {
        key: "daily_reminder_enabled",
        value: "false",
      }),
    );
  });
});

describe("ShowReceiptLine", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  function mockSettings(values: Record<string, string | null>) {
    invalidateCommands();
    mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "get_app_info") return Promise.resolve(APP_INFO);
      if (cmd === "get_app_setting") {
        const key = String(args?.["key"]);
        return Promise.resolve(key in values ? values[key] : null);
      }
      if (cmd === "set_app_setting") return Promise.resolve(undefined);
      return Promise.reject(new Error(`unmocked command: ${cmd}`));
    });
  }

  it("shows the receipt toggle in the default ON state when the key is absent", async () => {
    mockSettings({}); // chave ausente → ligado por padrão
    renderSettings();

    const toggle = await screen.findByRole("switch", { name: "Conta sempre à mostra" });
    expect(toggle).toHaveAttribute("aria-checked", "true");
  });

  it("persists the toggle off", async () => {
    const user = userEvent.setup();
    mockSettings({});
    renderSettings();

    const toggle = await screen.findByRole("switch", { name: "Conta sempre à mostra" });
    await user.click(toggle);

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("set_app_setting", {
        key: "mia_show_receipt",
        value: "false",
      }),
    );
  });
});

// Estes testes atravessam a costura backend↔tela: os fixtures usam exatamente o que os
// comandos Rust (snapshot_cmds.rs) realmente devolvem — RFC3339 em `last_checkin_at`, a
// mensagem verbatim da recusa do lease, o `published: false` do veredito "em dia" — em vez de
// um formato conveniente só assumido pelo teste.
describe("DriveCheckinLine — check-in do snapshot no Drive", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("mostra a recusa do lease VERBATIM, não o erro genérico", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_app_info: APP_INFO,
      last_drive_checkin: {
        last_checkin_at: null,
        last_checkin_device_id: null,
        this_device_id: "aparelho-a",
      },
      drive_checkin: new Error(
        "Outro aparelho publicou depois do seu último check-in. Baixe a versão mais recente antes de subir a sua.",
      ),
    });
    renderSettings();

    await user.click(await screen.findByRole("button", { name: "Fazer check-in" }));

    expect(
      await screen.findByText(
        "Outro aparelho publicou depois do seu último check-in. Baixe a versão mais recente antes de subir a sua.",
      ),
    ).toBeInTheDocument();
  });

  it("'em dia' é sucesso: copy calma, nunca role=alert", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_app_info: APP_INFO,
      last_drive_checkin: {
        last_checkin_at: "2026-08-11T10:00:00+00:00",
        last_checkin_device_id: "aparelho-a",
        this_device_id: "aparelho-a",
      },
      drive_checkin: {
        last_checkin_at: "2026-08-11T10:00:00+00:00",
        last_checkin_device_id: "aparelho-a",
        this_device_id: "aparelho-a",
        published: false,
      },
    });
    renderSettings();

    await user.click(await screen.findByRole("button", { name: "Fazer check-in" }));

    const note = await screen.findByText("Já está em dia — nada novo para publicar.");
    // "Em dia" é o caso normal, não uma falha: a copy não pode viver dentro de uma região
    // role="alert" (regra 16 de docs/ui-standards.md).
    expect(note.closest('[role="alert"]')).toBeNull();
  });

  it("mostra a recência do último check-in a partir do timestamp real do comando (RFC3339)", async () => {
    mockCommands({
      get_app_info: APP_INFO,
      last_drive_checkin: {
        last_checkin_at: "2026-08-11T14:55:00+00:00",
        last_checkin_device_id: "aparelho-a",
        this_device_id: "aparelho-a",
      },
    });
    renderSettings();

    expect(await screen.findByText(/Último check-in/)).toBeInTheDocument();
  });
});
