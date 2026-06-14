import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { OnboardingFlow } from "./OnboardingFlow";
import { mockCommands, mockInvoke } from "../../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("OnboardingFlow", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("avança pelos 5 passos e persiste onboarding_done ao concluir", async () => {
    const user = userEvent.setup();
    mockCommands({ set_app_setting: null, list_tags_cmd: [] });
    const onDone = vi.fn();
    render(<OnboardingFlow onDone={onDone} />);

    // Passo 1 (boas-vindas) mostra os tipos do método.
    expect(screen.getByText("Bem-vindo ao Neko")).toBeInTheDocument();
    expect(screen.getByText("1 / 5")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Avançar/ }));
    expect(screen.getByText(/Previsível > categorizar/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Avançar/ }));
    expect(screen.getByText("Traga seus dados")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Avançar/ }));
    expect(screen.getByText("Seu primeiro lançamento")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Avançar/ }));
    expect(screen.getByText("Sua meta de poupança")).toBeInTheDocument();

    // Último passo: "Começar" persiste e fecha.
    await user.click(screen.getByRole("button", { name: /Começar/ }));
    await waitFor(() => expect(onDone).toHaveBeenCalledTimes(1));
    const setCall = mockInvoke.mock.calls.find((c) => c[0] === "set_app_setting");
    expect(setCall?.[1]).toMatchObject({ key: "onboarding_done", value: "true" });
  });

  it("Pular persiste e fecha imediatamente", async () => {
    const user = userEvent.setup();
    mockCommands({ set_app_setting: null });
    const onDone = vi.fn();
    render(<OnboardingFlow onDone={onDone} />);

    await user.click(screen.getByRole("button", { name: "Pular" }));
    await waitFor(() => expect(onDone).toHaveBeenCalledTimes(1));
    expect(mockInvoke.mock.calls.some((c) => c[0] === "set_app_setting")).toBe(true);
  });
});
