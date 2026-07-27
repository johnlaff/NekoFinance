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
});
