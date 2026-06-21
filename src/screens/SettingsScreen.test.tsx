import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { SettingsScreen } from "./SettingsScreen";
import { APP_INFO, POCKETS, mockCommands, mockInvoke } from "../test/commands";
import { invalidateCommands } from "../lib/useCommand";
import { open } from "@tauri-apps/plugin-dialog";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

const mockOpen = open as ReturnType<typeof vi.fn>;

describe("SettingsScreen", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockOpen.mockReset();
  });

  it("shows the local data location and version", async () => {
    mockCommands({ get_app_info: APP_INFO });
    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(APP_INFO.db_path)).toBeInTheDocument();
    });
    expect(screen.getByText(/v0\.1\.0/)).toBeInTheDocument();
    expect(screen.getByText(/não envia nenhum dado/)).toBeInTheDocument();
  });

  it("imports a local xlsx through the native dialog and reports the result", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_app_info: APP_INFO,
      import_local_xlsx: "Imported 12 total rows from: 2026 (12 rows)",
    });
    mockOpen.mockResolvedValue("/home/user/financas.xlsx");

    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);

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

    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);
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

    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: /Escolher arquivo/ }));

    await waitFor(() => {
      expect(
        screen.getByText(/Não foi possível importar o arquivo local/),
      ).toBeInTheDocument();
    });
  });

  it("lists pockets with PT-BR type labels (spec 007)", async () => {
    mockCommands({ get_app_info: APP_INFO, get_pockets: POCKETS });
    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);

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
    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);

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
    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);

    await user.type(screen.getByLabelText("Nome"), "Conta");
    await user.type(screen.getByLabelText("Saldo (R$)"), "abc");
    await user.click(screen.getByRole("button", { name: /Adicionar bolso/ }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/Saldo inválido/);
    expect(mockInvoke).not.toHaveBeenCalledWith("create_account", expect.anything());
  });

  it("offers the Google connect flow when disconnected", async () => {
    mockCommands({ get_app_info: APP_INFO });
    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);
    expect(
      await screen.findByRole("button", { name: /Conectar Google/ }),
    ).toBeInTheDocument();
  });

  it("DailyTetoCeilingSection: mostra o campo de teto e chama upsert_daily_budget ao salvar", async () => {
    const user = userEvent.setup();
    // isTauri é true no ambiente de teste (setup.ts define window.__TAURI_INTERNALS__),
    // então a seção renderiza. get_app_setting=null deixa o campo vazio na montagem.
    mockCommands({
      get_app_info: APP_INFO,
      get_app_setting: null,
      set_app_setting: undefined,
      upsert_daily_budget: undefined,
    });
    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);

    const input = await screen.findByLabelText("Teto diário em reais");
    await user.type(input, "50,00");
    await user.click(screen.getByRole("button", { name: "Salvar" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "upsert_daily_budget",
        expect.objectContaining({ amountCents: 5000 }),
      );
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
      // Agendamento no nível do sistema (plano 039): melhor-esforço; aqui sempre resolve.
      if (cmd === "register_os_reminder" || cmd === "unregister_os_reminder")
        return Promise.resolve(undefined);
      return Promise.reject(new Error(`unmocked command: ${cmd}`));
    });
  }

  it("shows the reminder toggle in the default ON state when the key is absent", async () => {
    mockSettings({}); // chaves ausentes → ligado por padrão
    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);

    await waitFor(() => {
      expect(
        screen.getByRole("radiogroup", { name: /lembrete diário/i }),
      ).toBeInTheDocument();
    });
    const on = screen.getByRole("radio", { name: "Ligado" });
    expect(on).toHaveAttribute("aria-checked", "true");
  });

  it("persists the toggle off", async () => {
    const user = userEvent.setup();
    mockSettings({});
    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);

    await waitFor(() =>
      expect(
        screen.getByRole("radiogroup", { name: /lembrete diário/i }),
      ).toBeInTheDocument(),
    );

    await user.click(screen.getByRole("radio", { name: "Desligado" }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("set_app_setting", {
        key: "daily_reminder_enabled",
        value: "false",
      }),
    );
  });
});
