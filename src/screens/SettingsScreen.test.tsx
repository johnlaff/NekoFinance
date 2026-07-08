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
      import_local_xlsx: {
        count: 12,
        summary: "Imported 12 total rows from: 2026 (12 rows)",
        diagnostics: [],
      },
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

  it("DiarioCategorySection: estado vazio mostra o botão de adicionar categoria", async () => {
    mockCommands({
      get_app_info: APP_INFO,
      get_app_setting: null,
      set_app_setting: undefined,
      get_daily_budget_categories_cmd: [],
      upsert_daily_budget_with_categories_cmd: undefined,
    });
    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);

    expect(
      await screen.findByRole("button", { name: "Adicionar categoria" }),
    ).toBeInTheDocument();
    // Resumo derivado com 0 categorias: total R$ 0,00 (dentro de um <Money>, então
    // casamos pelo textContent do <p> em vez do texto de um único nó).
    expect(
      screen.getByText(
        (_, el) =>
          el?.tagName === "P" &&
          (el.textContent ?? "").replace(/\s+/g, " ").includes("Total R$ 0,00"),
      ),
    ).toBeInTheDocument();
  });

  it("DiarioCategorySection: renderiza as categorias existentes (nome + valor)", async () => {
    mockCommands({
      get_app_info: APP_INFO,
      get_app_setting: "1.250,00",
      set_app_setting: undefined,
      get_daily_budget_categories_cmd: [
        { id: "c1", name: "Alimentação", amount_cents: 30000, position: 0 },
        { id: "c2", name: "Transporte", amount_cents: 20000, position: 1 },
      ],
      upsert_daily_budget_with_categories_cmd: undefined,
    });
    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);

    expect(await screen.findByDisplayValue("Alimentação")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Transporte")).toBeInTheDocument();
    expect(screen.getByDisplayValue("300,00")).toBeInTheDocument();
    expect(screen.getByDisplayValue("200,00")).toBeInTheDocument();
  });

  it("DiarioCategorySection: Salvar chama upsert_daily_budget_with_categories_cmd com os args", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_app_info: APP_INFO,
      get_app_setting: null,
      set_app_setting: undefined,
      get_daily_budget_categories_cmd: [],
      upsert_daily_budget_with_categories_cmd: undefined,
    });
    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);

    // Define o teto total e adiciona uma categoria.
    await user.type(
      await screen.findByLabelText("Teto mensal do Diário em reais"),
      "1.250,00",
    );
    await user.click(screen.getByRole("button", { name: "Adicionar categoria" }));
    await user.type(screen.getByLabelText("Nome da categoria 1"), "Alimentação");
    await user.type(
      screen.getByLabelText("Valor mensal da categoria 1 em reais"),
      "300,00",
    );
    await user.click(screen.getByRole("button", { name: "Salvar categorias" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "upsert_daily_budget_with_categories_cmd",
        {
          amountCents: 125000,
          categories: [{ name: "Alimentação", amount_cents: 30000, position: 0 }],
        },
      );
    });
  });

  it("DiarioCategorySection: mostra o teto/dia derivado (total ÷ dias do mês)", async () => {
    // Total = soma das categorias quando o teto fica em branco (60000 cents). O teto/dia depende
    // dos dias do mês ATUAL; calculamos o esperado pela mesma fórmula para não depender de relógio
    // fixo (que quebra `findBy*` sob fake timers).
    const now = new Date();
    const daysInMonth = new Date(now.getFullYear(), now.getMonth() + 1, 0).getDate();
    const expectedRate = Math.floor(60000 / daysInMonth); // cents/dia
    const reais = (expectedRate / 100).toFixed(2).replace(".", ",");

    mockCommands({
      get_app_info: APP_INFO,
      get_app_setting: null,
      set_app_setting: undefined,
      get_daily_budget_categories_cmd: [
        { id: "c1", name: "Alimentação", amount_cents: 60000, position: 0 },
      ],
      upsert_daily_budget_with_categories_cmd: undefined,
    });
    render(<SettingsScreen authStatus="disconnected" onAuthChange={vi.fn()} />);

    // O valor renderiza dentro de um <Money> (a11y), então o texto some da leitura
    // padrão de nó-a-nó do RTL — casamos pelo textContent completo do <p>.
    const rateRe = new RegExp(`R\\$ ${reais}/dia`);
    await waitFor(() => {
      expect(
        screen.getByText(
          (_, el) =>
            el?.tagName === "P" &&
            rateRe.test((el.textContent ?? "").replace(/\s+/g, " ")),
        ),
      ).toBeInTheDocument();
    });
    expect(
      screen.getByText(new RegExp(`${daysInMonth} dias no mês atual`)),
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
