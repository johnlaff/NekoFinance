import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { SettingsScreen } from "./SettingsScreen";
import { APP_INFO, POCKETS, mockCommands, mockInvoke } from "../test/commands";
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
      expect(screen.getByText(/corrupt file/)).toBeInTheDocument();
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
});
