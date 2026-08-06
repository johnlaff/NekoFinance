import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { DashboardScreen } from "./DashboardScreen";
import { NekoAppProvider } from "../shell/appContext";
import type { UpcomingInvoice } from "../lib/api";
import { FORECAST, SUMMARY, mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => undefined)),
}));

const app = { navigate: vi.fn(), openCompose: vi.fn() };

function renderHoje() {
  return render(
    <NekoAppProvider value={app}>
      <DashboardScreen />
    </NekoAppProvider>,
  );
}

function invoiceFixture(overrides: Partial<UpcomingInvoice>): UpcomingInvoice {
  return {
    account_id: "card-1",
    card_name: "Cartão",
    due_date: "2026-06-20",
    amount_cents: 100_00,
    status: "aberta",
    owner_name: "Eu",
    has_refund_expectation: false,
    refund_expected_cents: 0,
    ...overrides,
  };
}

describe("DashboardScreen (Hoje)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    app.navigate.mockReset();
    app.openCompose.mockReset();
  });

  it("herói: saudação, veredito com o guardrail que morde e curadoria da Mia", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    const { container } = renderHoje();

    expect(await screen.findByText(/Pode gastar hoje/)).toBeInTheDocument();
    // O fixture prende no guardrail de poupança — a segunda linha nomeia ELE.
    expect(
      screen.getByText("Sem tocar na economia planejada do ano."),
    ).toBeInTheDocument();
    expect(screen.getByText(/A Mia separou o que importa hoje/)).toBeInTheDocument();
    // Primeiro adotante da coordenação large-title do shell.
    expect(container.querySelector("[data-large-title]")).not.toBeNull();
  });

  it("o registro inline morreu: o bloco do dia é leitura; registrar vive no shell", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    await screen.findByText(/Pode gastar hoje/);
    expect(screen.queryByRole("radiogroup")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Registrar" })).not.toBeInTheDocument();
  });

  it("modo débito: check-in do teto com barra e valor do teto", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    expect(await screen.findByText("Diário de hoje")).toBeInTheDocument();
    // O número exibido é só o guardrail; a mecânica completa (teto = segundo
    // limite) mora na didática recolhida "Como funciona?" — padrão do método.
    expect(screen.getByText("Como funciona?")).toBeInTheDocument();
    // O teto informado vira rótulo curto tocável (leva à tela do teto).
    expect(screen.getByRole("button", { name: /por dia/ })).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: /Diário de hoje em \d+% do teto/ }),
    ).toBeInTheDocument();
  });

  it("teto estimado: número com selo de estimativa, nunca veredito silencioso", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        daily_budget: 10000,
        daily_ceiling_source: "estimate",
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    expect(await screen.findByText(/Pode gastar hoje/)).toBeInTheDocument();
    expect(screen.getAllByText("Estimativa").length).toBeGreaterThanOrEqual(1);
  });

  it("sem teto + proposta pendente: convite único de revisão levando à cerimônia", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        daily_budget: 0,
        daily_ceiling_source: "none",
        ceiling_proposal_pending: true,
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    const review = await screen.findByRole("button", {
      name: "Proposta do teto — revisar.",
    });
    // Com proposta pendente o convite é ÚNICO na tela inteira: nenhum "estipular"
    // sobra, e a MESMA frase aparece nos dois pontos (herói e denominador).
    expect(screen.queryByText(/estipular/i)).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Proposta do teto — revisar" }),
    ).toBeInTheDocument();
    await user.click(review);
    expect(app.navigate).toHaveBeenCalledWith("teto");
  });

  // A faixa 20–30% é média ANUAL. Rompida, ela sai do teto — travar o dia puniria um déficit
  // que nenhum gasto de hoje desfaz — mas o diagnóstico continua na tela apontando o caminho.
  it("economia abaixo do piso não zera o teto, e o diagnóstico fica visível", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: {
        ...FORECAST,
        safe_to_spend_today_cents: 734_608,
        binding_guardrail: "cash",
        cash_headroom_cents: 734_608,
        savings_headroom_cents: -1_695_966,
        savings_band_verdict: "below_band",
        deepest_deficit: { date: "2026-08-12", balance_cents: 734_608 },
      },
    });
    renderHoje();
    expect(
      await screen.findByText("Sem deixar nenhum dia no vermelho."),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/economia do ano está abaixo dos 20%/i),
    ).toBeInTheDocument();
    expect(screen.getByText(/performance do mês/i)).toBeInTheDocument();
  });

  // Com a faixa viva, a régua da economia volta a mandar — ela protege quem ainda está nela.
  it("faixa viva mantém a economia como régua que morde", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: {
        ...FORECAST,
        safe_to_spend_today_cents: 50_000,
        binding_guardrail: "savings",
        savings_headroom_cents: 50_000,
      },
    });
    renderHoje();
    expect(
      await screen.findByText("Sem tocar na economia planejada do ano."),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/economia do ano está abaixo dos 20%/i),
    ).not.toBeInTheDocument();
  });

  // 22% está DENTRO da faixa (piso 20%) — a folga pode ser pequena, mas o veredito publicado
  // é quem decide, não o sinal de savings_headroom_cents. Este é o falso positivo histórico: um
  // arredondamento pode deixar a folga negativa mesmo com o percentual dentro da faixa — e é
  // exatamente esse descolamento entre sinal e veredito que a tela não pode mais reproduzir.
  it("22% dentro da faixa não mostra o diagnóstico de abaixo dos 20%, mesmo com a folga negativa por arredondamento", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: {
        ...FORECAST,
        binding_guardrail: "cash",
        savings_headroom_cents: -50,
        savings_band_verdict: "in_band",
        deepest_deficit: { date: "2026-08-12", balance_cents: 734_608 },
      },
    });
    renderHoje();
    expect(
      await screen.findByText("Sem deixar nenhum dia no vermelho."),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/economia do ano está abaixo dos 20%/i),
    ).not.toBeInTheDocument();
  });

  // above_band é faixa viva — mesmo tratamento de in_band, sem diagnóstico.
  it("acima da faixa também não mostra diagnóstico", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: {
        ...FORECAST,
        safe_to_spend_today_cents: 50_000,
        binding_guardrail: "savings",
        savings_headroom_cents: 50_000,
        savings_band_verdict: "above_band",
      },
    });
    renderHoje();
    expect(
      await screen.findByText("Sem tocar na economia planejada do ano."),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/economia do ano está abaixo dos 20%/i),
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/é a troca certa/i)).not.toBeInTheDocument();
  });

  // no_record: nada para diagnosticar ainda — nem "abaixo dos 20%" nem "troca certa".
  it("sem registro do ano não mostra nenhum diagnóstico da faixa", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: {
        ...FORECAST,
        binding_guardrail: "cash",
        savings_headroom_cents: null,
        savings_band_verdict: "no_record",
        deepest_deficit: { date: "2026-08-12", balance_cents: 734_608 },
      },
    });
    renderHoje();
    expect(
      await screen.findByText("Sem deixar nenhum dia no vermelho."),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/economia do ano está abaixo dos 20%/i),
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/é a troca certa/i)).not.toBeInTheDocument();
  });

  // Zero-por-escolha é a ordem do método cumprida — a cópia chama pelo nome e não usa
  // linguagem de falta.
  it("zero por escolha com reserva de pé mostra 'é a troca certa', nunca frase de falta", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: {
        ...FORECAST,
        safe_to_spend_today_cents: 734_608,
        binding_guardrail: "cash",
        cash_headroom_cents: 734_608,
        savings_headroom_cents: 0,
        savings_band_verdict: "zero_by_choice",
        deepest_deficit: { date: "2026-08-12", balance_cents: 734_608 },
      },
    });
    renderHoje();
    expect(await screen.findByText(/é a troca certa/i)).toBeInTheDocument();
    expect(
      screen.queryByText(/economia do ano está abaixo dos 20%/i),
    ).not.toBeInTheDocument();
    // Nenhuma frase de falta de economia — a única "falta" na tela é a do retrato da reserva,
    // um card diferente que não faz parte do diagnóstico da faixa.
    expect(
      screen.queryByText(/faltam .* de guardar|faltando guardar/i),
    ).not.toBeInTheDocument();
  });

  // A ponte que o método ensina: saldo negativo é o momento de ACIONAR a reserva. O gesto
  // pré-preenche a Entrada — o lançamento continua sendo do dono, nunca automático.
  it("déficit com reserva disponível oferece o saque pré-preenchido", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        reserve_state: "verdict",
        reserve_months: 7,
      },
      get_forecast: {
        ...FORECAST,
        safe_to_spend_today_cents: 0,
        binding_guardrail: "cash",
        deepest_deficit: { date: "2026-06-14", balance_cents: -100_000 },
      },
    });
    renderHoje();
    await user.click(await screen.findByRole("button", { name: "lançar o saque" }));
    expect(app.openCompose).toHaveBeenCalledWith({
      mode: "new",
      type: "entrada",
      date: "2026-06-14",
      description: "Saque da reserva de emergência",
      amountCents: 100_000,
    });
  });

  // Sem reserva mapeada o conselho muda: sugerir um saque impossível seria conselho vazio, e o
  // método aponta para a performance do mês.
  it("déficit sem reserva aponta para a performance, não para um saque", async () => {
    mockCommands({
      get_dashboard_summary: { ...SUMMARY, reserve_state: "no_record" },
      get_forecast: {
        ...FORECAST,
        safe_to_spend_today_cents: 0,
        binding_guardrail: "cash",
        deepest_deficit: { date: "2026-06-14", balance_cents: -100_000 },
      },
    });
    renderHoje();
    expect(await screen.findByText(/performance do mês/i)).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "lançar o saque" }),
    ).not.toBeInTheDocument();
  });

  // Alcançado o alvo, a pergunta do método muda: deixa de ser "quanto falta" e passa a ser o
  // que fazer com o excedente — é ele que financia o próximo movimento.
  it("reserva acima do alvo mostra o excedente", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        reserve_state: "verdict",
        reserve_months: 8.2,
        reserve_basis_months: 6,
        reserve_target_cents: 6_822_486,
        reserve_surplus_cents: 1_500_000,
      },
      get_forecast: FORECAST,
    });
    renderHoje();
    expect(await screen.findByText(/além do alvo/i)).toBeInTheDocument();
  });

  it("reserva ainda em construção não fabrica excedente", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        reserve_state: "estimate",
        reserve_months: 2.1,
        reserve_surplus_cents: null,
      },
      get_forecast: FORECAST,
    });
    renderHoje();
    await screen.findByText("Reserva de emergência");
    expect(screen.queryByText(/além do alvo/i)).not.toBeInTheDocument();
  });

  // Uma reserva incompleta NÃO aperta o teto: no método ela socorre o vermelho, não o proíbe.
  // Com o saldo no azul a leitura é a de caixa comum, mesmo sem nada guardado.
  it("reserva incompleta não zera o teto do dia", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: {
        ...FORECAST,
        binding_guardrail: "cash",
        cash_headroom_cents: 734_608,
        deepest_deficit: { date: "2026-06-12", balance_cents: 734_608 },
      },
    });
    renderHoje();
    expect(
      await screen.findByText("Sem deixar nenhum dia no vermelho."),
    ).toBeInTheDocument();
    expect(screen.queryByText(/reserva de emergência está/i)).not.toBeInTheDocument();
  });

  // Quando o mês abre o bico, o método manda ACIONAR a reserva — a tela aponta para o gesto.
  it("saldo que fura o zero aponta para o saque da reserva", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: {
        ...FORECAST,
        safe_to_spend_today_cents: 0,
        binding_guardrail: "cash",
        deepest_deficit: { date: "2026-06-14", balance_cents: -100_000 },
      },
    });
    renderHoje();
    expect(
      await screen.findByText("O mês já abre o bico — o teto de hoje é zero."),
    ).toBeInTheDocument();
    expect(screen.getByText(/é para isso que a reserva existe/i)).toBeInTheDocument();
  });

  it("modo cartão: faturas em aberto agrupadas por vencimento são o corpo do bloco", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        spending_mode: "card",
        card_spend_today_cents: 0,
        upcoming_invoices: [
          invoiceFixture({
            account_id: "itau",
            card_name: "Itaú",
            amount_cents: 1_747_39,
            due_date: "2026-06-20",
          }),
          invoiceFixture({
            account_id: "amazon",
            card_name: "Amazon",
            amount_cents: 195_62,
            due_date: "2026-06-20",
          }),
          invoiceFixture({
            account_id: "gio",
            card_name: "Bradesco Gio",
            amount_cents: 987_70,
            due_date: "2026-06-22",
            owner_name: "Gio",
            has_refund_expectation: true,
            refund_expected_cents: 987_70,
          }),
        ],
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [
        { id: "itau", name: "Itaú" },
        { id: "amazon", name: "Amazon" },
        { id: "gio", name: "Bradesco Gio" },
        { id: "inter", name: "Inter" },
        { id: "bb", name: "BB" },
      ],
    });
    renderHoje();

    // Cabeçalhos por vencimento, com concordância singular/plural.
    expect(await screen.findByText("Vencem em 20 de junho")).toBeInTheDocument();
    expect(screen.getByText("Vence em 22 de junho")).toBeInTheDocument();
    // Dentro do grupo, maior primeiro; a maior de todas soma o destaque ao próprio status.
    expect(screen.getByText(/· a maior fatura em aberto/)).toBeInTheDocument();
    // Reembolso previsto vira etiqueta com valor; o dono aparece quando há mais de um.
    expect(screen.getByText("Reembolso:", { exact: false })).toBeInTheDocument();
    expect(screen.getByText(/De Gio/)).toBeInTheDocument();
    // Cartão parado nunca some em silêncio.
    expect(
      screen.getByText(/Inter e BB estão sem fatura em aberto/),
    ).toBeInTheDocument();
    // A régua de Diário sai de cena no modo cartão.
    expect(screen.queryByText("Diário de hoje")).not.toBeInTheDocument();
    expect(screen.getByText(/nada somado à fatura hoje/)).toBeInTheDocument();
    // A didática do velocímetro mora no cabeçalho tocável; só o dado fica inline.
    expect(screen.getByText(/Até aqui: \d+% do gasto típico/)).toBeInTheDocument();
    expect(screen.queryByText(/é o velocímetro de quem gasta/)).not.toBeInTheDocument();
    // "Ver tudo" das faturas leva à tela Cartões, não ao livro-razão.
    expect(
      screen.getByRole("button", { name: "Ver tudo — faturas dos cartões" }),
    ).toBeInTheDocument();
  });

  it("modo cartão: a maior fatura continua dizendo se acumula ou aguarda pagamento", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        spending_mode: "card",
        upcoming_invoices: [
          invoiceFixture({
            account_id: "maior",
            card_name: "Maior",
            amount_cents: 350_894,
            status: "aberta",
          }),
          invoiceFixture({
            account_id: "menor",
            card_name: "Menor",
            amount_cents: 20_485,
            status: "fechada",
          }),
        ],
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    // O destaque de tamanho é julgamento; o status é fato. Um não pode apagar o outro.
    expect(
      await screen.findByText(/Acumulando · a maior fatura em aberto/),
    ).toBeInTheDocument();
    expect(screen.getByText("Fechada — aguarda pagamento")).toBeInTheDocument();
  });

  it("modo cartão: o status e a etiqueta de reembolso não colam no texto da linha", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        spending_mode: "card",
        upcoming_invoices: [
          invoiceFixture({
            account_id: "titular",
            card_name: "Titular",
            amount_cents: 350_894,
            status: "aberta",
          }),
          invoiceFixture({
            account_id: "adicional",
            card_name: "Adicional",
            amount_cents: 153_239,
            status: "fechada",
            owner_name: "Gio",
            has_refund_expectation: true,
            refund_expected_cents: 153_239,
          }),
        ],
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    // A margem do CSS separa aos olhos, mas não ao leitor de tela nem ao texto copiado:
    // sem separador no conteúdo, "pagamento" e "Reembolso" viram uma palavra só.
    const linha = (await screen.findByText("Adicional")).closest("li");
    expect(linha?.textContent).not.toMatch(/pagamentoReembolso/);
    expect(linha?.textContent).toMatch(/aguarda pagamento\s+·\s+Reembolso:/);
  });

  it("modo cartão: mostra o total líquido e a parte que volta como reembolso", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        spending_mode: "card",
        upcoming_invoices: [
          invoiceFixture({
            account_id: "titular",
            card_name: "Cartão titular",
            amount_cents: 200_00,
            refund_expected_cents: 50_00,
          }),
          invoiceFixture({
            account_id: "adicional",
            card_name: "Cartão adicional",
            amount_cents: 100_00,
            refund_expected_cents: 100_00,
          }),
        ],
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    expect(
      await screen.findByText(/Faturas em aberto — 2 cartões/),
    ).toBeInTheDocument();
    expect(screen.getAllByText(/R\$\s*150,00/)).toHaveLength(2);
    expect(screen.getByText("Já descontado:", { exact: false })).toHaveTextContent(
      "Já descontado: R$ 150,00 que volta como reembolso.",
    );
    expect(screen.getAllByText("Reembolso:", { exact: false })).toHaveLength(2);
  });

  it("modo cartão: com um reembolso, deixa o valor apenas na fatura", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        spending_mode: "card",
        upcoming_invoices: [
          invoiceFixture({
            amount_cents: 100_00,
            refund_expected_cents: 50_00,
          }),
        ],
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    expect(await screen.findByText(/Faturas em aberto — 1 cartão/)).toBeInTheDocument();
    expect(
      screen.queryByText("Já descontado:", { exact: false }),
    ).not.toBeInTheDocument();
    expect(screen.getByText("Reembolso:", { exact: false })).toBeInTheDocument();
  });

  it("modo cartão sem reembolso não sugere uma devolução", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        spending_mode: "card",
        upcoming_invoices: [
          invoiceFixture({
            amount_cents: 100_00,
            has_refund_expectation: true,
            refund_expected_cents: 0,
          }),
        ],
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    await screen.findByText(/Faturas em aberto/);
    expect(screen.queryByText(/reembolso/i)).not.toBeInTheDocument();
  });

  it("fatura zerada some da lista e explica o cartão que fica sem fatura em aberto", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        spending_mode: "card",
        upcoming_invoices: [
          invoiceFixture({
            account_id: "parado",
            card_name: "Cartão parado",
            amount_cents: 0,
            status: "fechada",
          }),
          invoiceFixture({
            account_id: "ativo",
            card_name: "Cartão ativo",
            amount_cents: 80_00,
          }),
        ],
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [
        { id: "parado", name: "Cartão parado" },
        { id: "ativo", name: "Cartão ativo" },
      ],
    });
    renderHoje();

    expect(await screen.findByText("Cartão ativo")).toBeInTheDocument();
    expect(screen.queryByText("Cartão parado")).not.toBeInTheDocument();
    expect(
      screen.getByText(/Cartão parado está sem fatura em aberto/),
    ).toBeInTheDocument();
  });

  it("limite do caixa cita o dia do menor saldo no mês e o mês quando ele está à frente", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: {
        ...FORECAST,
        binding_guardrail: "cash",
        deepest_deficit: { date: "2026-06-15", balance_cents: 587_700 },
      },
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    const currentMonth = renderHoje();

    expect(
      await screen.findByText(/até 15 de junho sem nenhum dia no vermelho/),
    ).toBeInTheDocument();
    currentMonth.unmount();

    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: {
        ...FORECAST,
        binding_guardrail: "cash",
        deepest_deficit: { date: "2026-09-03", balance_cents: 587_700 },
      },
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    expect(
      await screen.findByText(/até 3 de setembro sem nenhum dia no vermelho/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/o ponto mais apertado do horizonte está em setembro/),
    ).toBeInTheDocument();
  });

  it("leituras de fatura e horizonte preservam o saldo projetado e o pode gastar hoje", async () => {
    const scenarios = [
      {
        summary: {
          ...SUMMARY,
          spending_mode: "card" as const,
          upcoming_invoices: [invoiceFixture({ amount_cents: 0 })],
        },
        forecast: FORECAST,
      },
      {
        summary: {
          ...SUMMARY,
          spending_mode: "card" as const,
          upcoming_invoices: [
            invoiceFixture({ amount_cents: 100_00, refund_expected_cents: 100_00 }),
          ],
        },
        forecast: FORECAST,
      },
      {
        summary: SUMMARY,
        forecast: {
          ...FORECAST,
          binding_guardrail: "cash" as const,
          deepest_deficit: { date: "2026-09-03", balance_cents: 587_700 },
        },
      },
    ];

    for (const scenario of scenarios) {
      mockCommands({
        get_dashboard_summary: scenario.summary,
        get_forecast: scenario.forecast,
        get_upcoming_bills_cmd: [],
        list_cards: [],
      });
      const screenView = renderHoje();

      expect(await screen.findByText(/Pode gastar hoje/)).toHaveTextContent(
        "R$ 350,00",
      );
      expect(screen.getByLabelText("Leitura da Mia")).toHaveTextContent(
        "saldo previsto de R$ 12.877,00",
      );
      screenView.unmount();
    }
  });

  it("reserva em retrato vivo: a Mia diz quantos meses já existem e quantos faltam", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        reserve_state: "estimate",
        reserve_basis_months: 4,
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    const insight = await screen.findByLabelText("Leitura da Mia sobre a reserva");
    // O selo "Estimativa" explica o conceito; a Mia entrega o DADO da janela.
    expect(insight).toHaveTextContent("4 de 6 meses completos");
    expect(insight).toHaveTextContent("faltam 2 meses");
  });

  it("modo cartão sem fatura em aberto: recai no Cartão do mês, nunca lista vazia muda", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        spending_mode: "card",
        cartao_month_cents: 260000,
        next_fatura_date: "2026-06-20",
        next_fatura_amount_cents: 140000,
        upcoming_invoices: [],
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [
        { id: "inter", name: "Inter" },
        { id: "bb", name: "BB" },
      ],
    });
    renderHoje();

    expect(await screen.findByText("Cartão do mês")).toBeInTheDocument();
    expect(screen.getByText(/Próxima fatura:/)).toBeInTheDocument();
    expect(screen.getByText("Modo cartão")).toBeInTheDocument();
    // Cartão parado nunca some em silêncio — mesmo sem NENHUMA fatura em aberto.
    expect(
      screen.getByText(/Inter e BB estão sem fatura em aberto/),
    ).toBeInTheDocument();
  });

  it("insight da Mia: fechamento na faixa do termômetro + ponto mais apertado + entrada", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    const insight = await screen.findByLabelText("Leitura da Mia");
    expect(insight).toHaveTextContent(/junho termina em Folga/);
    expect(insight).toHaveTextContent(/O ponto mais apertado do mês é dia 15/);
    expect(insight).toHaveTextContent(/a próxima entrada chega dia 25/);
    expect(insight).toHaveTextContent(/Nenhum dia no vermelho à vista/);
  });

  it("próximos movimentos: contas e a próxima entrada, em ordem de data", async () => {
    mockCommands({
      get_dashboard_summary: SUMMARY,
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [
        {
          id: "bill-1",
          description: "Aulas de inglês",
          amount: 23200,
          due_date: "2026-06-30",
          is_projection: true,
        },
      ],
      list_cards: [],
    });
    renderHoje();

    const list = await screen.findByRole("list", { name: "Próximos movimentos" });
    const items = [...list.querySelectorAll("li")].map((li) => li.textContent ?? "");
    // Entrada de 25/06 vem antes da conta de 30/06.
    expect(items[0]).toContain("Entrada prevista");
    expect(items[1]).toContain("Aulas de inglês");
  });

  it("mostra estado de erro com retry quando o fetch falha — nunca R$ 0,00 como dado real", async () => {
    mockCommands({
      get_dashboard_summary: new Error("db offline"),
      get_forecast: new Error("db offline"),
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    expect(
      await screen.findByText("Não foi possível carregar o painel"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Tentar novamente" }),
    ).toBeInTheDocument();
    // Sem dados reais, o herói e qualquer "R$ 0,00" fabricado não podem aparecer.
    expect(screen.queryByText(/Pode gastar hoje/)).not.toBeInTheDocument();
    expect(screen.queryByText(/R\$\s?0,00/)).not.toBeInTheDocument();
  });

  it("estado de erro também quando SÓ uma das fontes falha no primeiro load (sem zeros fabricados)", async () => {
    mockCommands({
      get_dashboard_summary: new Error("db offline"),
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    expect(
      await screen.findByText("Não foi possível carregar o painel"),
    ).toBeInTheDocument();
    expect(screen.queryByText(/Pode gastar hoje/)).not.toBeInTheDocument();
    expect(screen.queryByText(/R\$\s?0,00/)).not.toBeInTheDocument();
  });

  it("reserva zerada: palavra dedicada em vez de alarme numérico", async () => {
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        reserve_months: 0,
        reserve_state: "zero",
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    expect(await screen.findByText("Sem reserva")).toBeInTheDocument();
    expect(screen.queryByText(/0,0 meses/)).not.toBeInTheDocument();
  });

  it("reserva sem registro: travessão + CTA de mapear levando a Configurações", async () => {
    const user = userEvent.setup();
    mockCommands({
      get_dashboard_summary: {
        ...SUMMARY,
        reserve_months: 0,
        reserve_state: "no_record",
      },
      get_forecast: FORECAST,
      get_upcoming_bills_cmd: [],
      list_cards: [],
    });
    renderHoje();

    expect(await screen.findByText("Sem registro")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Mapear" }));
    expect(app.navigate).toHaveBeenCalledWith("config");
  });
});
