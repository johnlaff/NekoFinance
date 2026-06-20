import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { PocketsManager } from "./PocketsManager";
import { POCKETS, EMPTY_POCKETS, mockCommands, mockInvoke } from "../../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// Characterization tests (plan 010): PocketsManager calls get_pockets on mount
// (isTauri is true in tests — setup.ts defines window.__TAURI_INTERNALS__). PIN the
// list render, empty state, form presence, and the error-mapping behavior.

describe("PocketsManager", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("renders_existing_pockets_list", async () => {
    mockCommands({ get_pockets: POCKETS });
    render(<PocketsManager />);
    await waitFor(() => expect(screen.getByText("Conta corrente")).toBeInTheDocument());
    expect(screen.getByText("Poupança")).toBeInTheDocument();
  });

  it("shows_nothing_above_form_when_no_pockets", async () => {
    mockCommands({ get_pockets: EMPTY_POCKETS });
    render(<PocketsManager />);
    // A lista só aparece com accounts.length > 0; aqui não deve haver nenhum item.
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());
    expect(document.querySelector(".pockets-list")).toBeNull();
    // O formulário continua presente.
    expect(screen.getByPlaceholderText("Ex.: Bolso demo")).toBeInTheDocument();
  });

  it("form_is_present_with_nome_and_tipo_fields", async () => {
    mockCommands({ get_pockets: EMPTY_POCKETS });
    render(<PocketsManager />);
    // Campo Nome (placeholder) + combobox de Tipo. isTauri=true → inputs habilitados.
    const nameInput = screen.getByPlaceholderText("Ex.: Bolso demo");
    expect(nameInput).toBeInTheDocument();
    expect(nameInput).not.toBeDisabled();
    expect(screen.getByRole("combobox")).toBeInTheDocument();
    // Deixa o get_pockets do mount resolver para não vazar update fora de act().
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());
  });

  it("shows_error_on_get_pockets_failure", async () => {
    // safeErrorMessage mapeia "locked" para a mensagem de banco ocupado (não o fallback).
    // Pinamos o comportamento ATUAL: "db locked" → mensagem específica de banco ocupado.
    mockCommands({ get_pockets: new Error("db locked") });
    render(<PocketsManager />);
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(/O banco local está ocupado/),
    );
  });

  it("shows_fallback_error_on_generic_get_pockets_failure", async () => {
    // Erro genérico (sem palavra-chave) → cai no fallback passado pelo componente.
    mockCommands({ get_pockets: new Error("boom") });
    render(<PocketsManager />);
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        /Não foi possível carregar os bolsos\./,
      ),
    );
  });
});
