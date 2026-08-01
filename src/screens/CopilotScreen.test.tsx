import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { CopilotScreen } from "./CopilotScreen";
import { resetSession } from "./miaSession";
import { NekoAppProvider } from "../shell/appContext";
import { FORECAST, SUMMARY, mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// O roteamento das perguntas, as contas dos recibos e as recusas são testados em
// `miaView.test.ts`. Aqui provamos que a tela monta os dados reais, escreve a conversa e
// entrega as saídas concretas de cada resposta.

const app = { navigate: vi.fn(), openCompose: vi.fn() };
const MIA_CONSENT = {
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
};

function renderMia() {
  return render(
    <NekoAppProvider value={app}>
      <CopilotScreen />
    </NekoAppProvider>,
  );
}

describe("CopilotScreen (Mia)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    app.navigate.mockReset();
    app.openCompose.mockReset();
    resetSession();
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_mia_consent: MIA_CONSENT,
    });
  });

  it("abre na saudação do gato, sem conversa fabricada", async () => {
    renderMia();
    expect(
      await screen.findByRole("heading", { name: /^(Bom dia|Boa tarde|Boa noite)\.$/ }),
    ).toBeInTheDocument();
    expect(screen.getByText(/Sou a Mia/)).toBeInTheDocument();
    // Nenhuma pergunta é colocada na boca da pessoa antes de ela falar.
    expect(screen.queryByText("Você:")).not.toBeInTheDocument();
    expect(screen.queryByText(/Cálculo determinístico/)).not.toBeInTheDocument();
  });

  it("responde a pergunta com o recibo da conta que o motor fez", async () => {
    const user = userEvent.setup();
    renderMia();
    await user.click(
      await screen.findByRole("button", { name: "Quanto posso gastar hoje?" }),
    );

    const log = screen.getByRole("log");
    expect(within(log).getByText("Quanto posso gastar hoje?")).toBeInTheDocument();
    expect(within(log).getByText("Limite do caixa")).toBeInTheDocument();
    expect(within(log).getByText("Pode gastar hoje")).toBeInTheDocument();
    // Proveniência declarada em toda resposta de cálculo.
    expect(within(log).getByText(/Cálculo determinístico/)).toBeInTheDocument();
    // O marco de dia abre a conversa.
    expect(within(log).getByText("Hoje")).toBeInTheDocument();
  });

  it("cada linha do painel faz a pergunta que a explica", async () => {
    const user = userEvent.setup();
    renderMia();
    await user.click(await screen.findByRole("button", { name: /Economizado no ano/ }));
    expect(
      within(screen.getByRole("log")).getByText("Como está a economia do ano?"),
    ).toBeInTheDocument();
  });

  it("pergunta fora do repertório recebe recusa honesta, não uma resposta inventada", async () => {
    const user = userEvent.setup();
    renderMia();
    const input = await screen.findByLabelText("Mensagem para a Mia");
    await user.type(input, "me conta uma piada{Enter}");

    const log = screen.getByRole("log");
    expect(within(log).getByText(/ainda não está ligada/)).toBeInTheDocument();
    expect(within(log).queryByText(/Cálculo determinístico/)).not.toBeInTheDocument();
  });

  it("não oferece ligar a conversa quando o consentimento e a chave já estão ligados", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_mia_consent: { ...MIA_CONSENT, granted: true, has_key: true, linked: true },
    });
    renderMia();
    const input = await screen.findByLabelText("Mensagem para a Mia");
    await user.type(input, "me conta uma piada{Enter}");

    expect(
      screen.queryByRole("button", { name: "Ligar a conversa" }),
    ).not.toBeInTheDocument();
  });

  it("a saída de cada recusa é concreta: tela certa ou o gesto de registrar", async () => {
    const user = userEvent.setup();
    renderMia();
    const input = await screen.findByLabelText("Mensagem para a Mia");

    await user.type(input, "onde gastei mais?{Enter}");
    await user.click(screen.getByRole("button", { name: "Abrir Tags" }));
    expect(app.navigate).toHaveBeenCalledWith("tags");

    await user.type(input, "registra 4,50 do café{Enter}");
    await user.click(screen.getByRole("button", { name: "Registrar lançamento" }));
    expect(app.openCompose).toHaveBeenCalled();
  });

  it("a conversa sobrevive à remontagem da tela (a navegação não apaga o que foi dito)", async () => {
    const user = userEvent.setup();
    const { unmount } = renderMia();
    await user.click(
      await screen.findByRole("button", { name: "Quanto posso gastar hoje?" }),
    );
    unmount();

    renderMia();
    expect(
      within(await screen.findByRole("log")).getByText("Quanto posso gastar hoje?"),
    ).toBeInTheDocument();
  });

  describe("mia_show_receipt", () => {
    it("ausente (default ligado): recibo inteiro visível, com o selo colado ao número", async () => {
      const user = userEvent.setup();
      renderMia();
      await user.click(
        await screen.findByRole("button", { name: "Como está a reserva?" }),
      );

      const log = screen.getByRole("log");
      expect(within(log).getByText("Meta do método")).toBeInTheDocument();
      expect(within(log).getByText("Reserva de hoje")).toBeInTheDocument();
      expect(within(log).getAllByText("Estimativa")).toHaveLength(1);
      expect(
        within(log).queryByRole("button", { name: "Ver a conta" }),
      ).not.toBeInTheDocument();
    });

    it("desligada: recolhe a conta, mantém o resultado e o selo, e expande sob pedido", async () => {
      const user = userEvent.setup();
      mockCommands({
        get_dashboard_summary: SUMMARY,
        get_forecast: FORECAST,
        get_mia_consent: MIA_CONSENT,
        get_app_setting: (args) =>
          (args as { key: string }).key === "mia_show_receipt" ? "false" : null,
      });
      renderMia();
      await user.click(
        await screen.findByRole("button", { name: "Como está a reserva?" }),
      );

      const log = screen.getByRole("log");
      // O resultado sobrevive ao recolhimento; o operando fica inerte até o gesto de abrir.
      expect(within(log).getByText("Reserva de hoje")).toBeInTheDocument();
      // A chave esconde aritmética, nunca estado do dado: o selo segue colado ao número.
      expect(within(log).getAllByText("Estimativa")).toHaveLength(1);
      const fold = within(log).getByText("Meta do método").closest(".nk-receipt__fold");
      expect(fold).toHaveAttribute("inert");

      const toggle = within(log).getByRole("button", { name: "Ver a conta" });
      expect(toggle).toHaveAttribute("aria-expanded", "false");

      await user.click(toggle);
      expect(fold).not.toHaveAttribute("inert");
      expect(
        within(log).getByRole("button", { name: "Ocultar a conta" }),
      ).toHaveAttribute("aria-expanded", "true");
    });

    it("operando estimado entre operandos vividos guarda o selo na própria linha", async () => {
      // O selo da frase qualifica o número que a resposta afirma. Quando é um operando que
      // está estimado, ele fica na linha da conta — a chave esconde aritmética, nunca o
      // estado do dado, e os dois selos convivem sem se confundir.
      const answer = {
        text: [{ t: "text", s: "A reserva cobre 4,5 meses do seu custo de vida." }],
        receipt: [
          {
            label: "Gasto típico",
            cents: 900_000,
            mark: {
              kind: "estimate",
              term: { title: "Retrato vivo", body: "Ainda é média." },
            },
          },
          { label: "Reserva de hoje", text: "4,5 meses", result: true },
        ],
        provenance: "calculo",
      };
      mockCommands({
        get_dashboard_summary: SUMMARY,
        get_forecast: FORECAST,
        get_mia_consent: MIA_CONSENT,
        load_mia_conversation: [
          {
            author: "voce",
            question: "Como está a reserva?",
            answer: null,
            at_iso: "2026-07-15T12:00",
          },
          { author: "mia", question: null, answer, at_iso: "2026-07-15T12:00" },
        ],
        get_app_setting: (args) =>
          (args as { key: string }).key === "mia_show_receipt" ? "false" : null,
      });
      renderMia();

      const log = await screen.findByRole("log");
      // Recolhida, o operando e o selo dele ficam inertes junto com o resto da conta.
      const fold = within(log).getByText("Gasto típico").closest(".nk-receipt__fold");
      expect(fold).toHaveAttribute("inert");
      expect(within(fold as HTMLElement).getByText("Estimativa")).toBeInTheDocument();

      await userEvent
        .setup()
        .click(within(log).getByRole("button", { name: "Ver a conta" }));
      expect(fold).not.toHaveAttribute("inert");
      expect(within(log).getAllByText("Estimativa")).toHaveLength(1);
    });

    it("desligada: recusa não tem recibo e não muda", async () => {
      const user = userEvent.setup();
      mockCommands({
        get_dashboard_summary: SUMMARY,
        get_forecast: FORECAST,
        get_mia_consent: MIA_CONSENT,
        get_app_setting: (args) =>
          (args as { key: string }).key === "mia_show_receipt" ? "false" : null,
      });
      renderMia();
      const input = await screen.findByLabelText("Mensagem para a Mia");
      await user.type(input, "onde gastei mais?{Enter}");

      const log = screen.getByRole("log");
      expect(
        within(log).queryByRole("button", { name: "Ver a conta" }),
      ).not.toBeInTheDocument();
      expect(
        await within(log).findByRole("button", { name: "Abrir Tags" }),
      ).toBeInTheDocument();
    });
  });

  describe("apagar conversa", () => {
    it("o botão só aparece quando há mensagens", async () => {
      renderMia();
      await screen.findByText(/Sou a Mia/);
      expect(
        screen.queryByRole("button", { name: "Apagar conversa" }),
      ).not.toBeInTheDocument();

      const user = userEvent.setup();
      await user.click(
        await screen.findByRole("button", { name: "Quanto posso gastar hoje?" }),
      );
      expect(
        await screen.findByRole("button", { name: "Apagar conversa" }),
      ).toBeInTheDocument();
    });

    it("confirm cancelado não apaga nada", async () => {
      const user = userEvent.setup();
      vi.spyOn(window, "confirm").mockReturnValue(false);
      renderMia();
      await user.click(
        await screen.findByRole("button", { name: "Quanto posso gastar hoje?" }),
      );

      await user.click(await screen.findByRole("button", { name: "Apagar conversa" }));

      expect(
        within(screen.getByRole("log")).getByText("Quanto posso gastar hoje?"),
      ).toBeInTheDocument();
      expect(mockInvoke).not.toHaveBeenCalledWith("delete_mia_conversation");
    });

    it("confirmado chama o comando de apagar e a tela volta ao vazio", async () => {
      const user = userEvent.setup();
      vi.spyOn(window, "confirm").mockReturnValue(true);
      mockCommands({
        get_dashboard_summary: SUMMARY,
        get_forecast: FORECAST,
        get_mia_consent: MIA_CONSENT,
        delete_mia_conversation: undefined,
      });
      renderMia();
      await user.click(
        await screen.findByRole("button", { name: "Quanto posso gastar hoje?" }),
      );

      await user.click(await screen.findByRole("button", { name: "Apagar conversa" }));

      expect(mockInvoke).toHaveBeenCalledWith("delete_mia_conversation");
      await screen.findByText(/Sou a Mia/);
      expect(
        screen.queryByRole("button", { name: "Apagar conversa" }),
      ).not.toBeInTheDocument();
    });
  });
});
