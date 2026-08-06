import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { TagsScreen } from "./TagsScreen";
import type {
  TagRulerEffects,
  TagRulerFlags,
  TagsScreenDto,
  TagsScreenTag,
  TagsScreenThirdParty,
} from "./tagsView";
import { mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const ALL_ON: TagRulerFlags = {
  performance: true,
  cost_of_living: true,
  savings: true,
  daily_avg: true,
};

const ZERO_EFFECTS: TagRulerEffects = {
  performance_delta_cents: 0,
  cost_delta_cents: 0,
  savings_base_delta_cents: 0,
  savings_amount_delta_cents: 0,
  daily_avg_delta_cents: 0,
};

function tag(
  overrides: Partial<TagsScreenTag> & { id: string; name: string },
): TagsScreenTag {
  return {
    color: "var(--cat-jade)",
    emoji: null,
    is_special: false,
    counts_in: ALL_ON,
    month_total_cents: 0,
    txn_count: 0,
    effects: ZERO_EFFECTS,
    ...overrides,
  };
}

function person(
  overrides: Partial<TagsScreenThirdParty> & {
    person_id: string;
    name: string;
    state: TagsScreenThirdParty["state"];
  },
): TagsScreenThirdParty {
  return {
    out_cents: 0,
    back_cents: 0,
    expected_cents: 0,
    open_since_days: null,
    series_done: null,
    series_total: null,
    settled_date: null,
    ...overrides,
  };
}

const GIO = tag({
  id: "gio",
  name: "Gio",
  color: "var(--cat-orchid)",
  counts_in: {
    performance: false,
    cost_of_living: false,
    savings: false,
    daily_avg: false,
  },
  month_total_cents: 407764,
  txn_count: 6,
  effects: {
    ...ZERO_EFFECTS,
    performance_delta_cents: 90000,
    cost_delta_cents: 407764,
  },
});

const TRANSITO = tag({
  id: "transito",
  name: "Trânsito",
  color: "var(--cat-sky)",
  counts_in: {
    performance: false,
    cost_of_living: false,
    savings: false,
    daily_avg: false,
  },
  month_total_cents: 100651,
  txn_count: 2,
  effects: { ...ZERO_EFFECTS, cost_delta_cents: 100651 },
});

const REEMBOLSO = tag({
  id: "reembolso",
  name: "Reembolso",
  color: "var(--cat-teal)",
  counts_in: {
    performance: true,
    cost_of_living: true,
    savings: false,
    daily_avg: true,
  },
  month_total_cents: 16700,
  txn_count: 3,
  effects: { ...ZERO_EFFECTS, savings_base_delta_cents: 16700 },
});

const MORADIA = tag({
  id: "moradia",
  name: "Moradia",
  color: "var(--cat-coral)",
  month_total_cents: 176656,
  txn_count: 2,
});

const EDUCACAO = tag({
  id: "educacao",
  name: "Educação",
  color: "var(--cat-violet)",
  month_total_cents: 54412,
  txn_count: 2,
});

const GIO_PERSON = person({
  person_id: "gio",
  name: "Gio",
  state: "favor",
  out_cents: 407764,
  back_cents: 497764,
});
const EDVALDO = person({
  person_id: "edvaldo",
  name: "Edvaldo",
  state: "open",
  out_cents: 5000,
  open_since_days: 13,
});
const PAI = person({
  person_id: "pai",
  name: "Pai",
  state: "series",
  back_cents: 11700,
  series_done: 2,
  series_total: 3,
});
const PABLO = person({
  person_id: "pablo",
  name: "Pablo",
  state: "settled",
  out_cents: 2200,
  back_cents: 2200,
  settled_date: "2026-07-04",
});
const BRUNA = person({ person_id: "bruna", name: "Bruna", state: "none" });

function dto(overrides: Partial<TagsScreenDto> = {}): TagsScreenDto {
  return {
    month: "2026-07",
    verdict: {
      cost_current_cents: 702873,
      cost_all_on_cents: 1211288,
      third_party_avg_cents: null,
      third_party_people: 0,
      has_exceptions: true,
    },
    third_parties: [],
    tags: [],
    last_sync_at: null,
    ...overrides,
  };
}

const RICH_DTO = dto({
  third_parties: [GIO_PERSON, EDVALDO, PAI, PABLO, BRUNA],
  tags: [GIO, TRANSITO, REEMBOLSO, MORADIA, EDUCACAO],
});

describe("TagsScreen", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date("2026-07-15T12:00:00-03:00"));
    mockInvoke.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // ---------------------------------------------------------------------
  // Estado A — ordem do DOM narra pela prova (veredito → terceiros →
  // exceções → rótulo).
  // ---------------------------------------------------------------------

  it("DOM order: veredito, dinheiro de terceiros, exceções e rótulo nessa ordem", async () => {
    mockCommands({ get_tags_screen: RICH_DTO });
    const { container } = render(<TagsScreen />);
    await screen.findByText("Exceções");

    const text = container.querySelector(".tags")!.textContent ?? "";
    const iVerdict = text.indexOf("Custo de vida · julho");
    const iTerceiros = text.indexOf("Dinheiro de terceiros");
    const iExcecoes = text.indexOf("Exceções");
    const iRotulo = text.indexOf("Movimentação por rótulo");

    expect(iVerdict).toBeGreaterThanOrEqual(0);
    expect(iVerdict).toBeLessThan(iTerceiros);
    expect(iTerceiros).toBeLessThan(iExcecoes);
    expect(iExcecoes).toBeLessThan(iRotulo);
  });

  it("agrupamento: tag com régua desligada é exceção; com as 4 ligadas é rótulo", async () => {
    mockCommands({ get_tags_screen: RICH_DTO });
    render(<TagsScreen />);
    await screen.findByText("Exceções");

    const excecoes = screen.getByText("Exceções").closest("section")!;
    expect(within(excecoes).getByText("Gio")).toBeInTheDocument();
    expect(within(excecoes).getByText("Trânsito")).toBeInTheDocument();
    expect(within(excecoes).getByText("Reembolso")).toBeInTheDocument();
    expect(within(excecoes).queryByText("Moradia")).not.toBeInTheDocument();
    expect(within(excecoes).queryByText("Educação")).not.toBeInTheDocument();

    const rotulo = screen.getByText("Movimentação por rótulo").closest("section")!;
    expect(within(rotulo).getByText("Moradia")).toBeInTheDocument();
    expect(within(rotulo).getByText("Educação")).toBeInTheDocument();
    expect(within(rotulo).queryByText("Gio")).not.toBeInTheDocument();
  });

  it("veredito A: número atual, excluído e a cauda 'sem as exceções' fecham com o DTO", async () => {
    mockCommands({ get_tags_screen: RICH_DTO });
    render(<TagsScreen />);
    // 7.028,73 = 12.112,88 − 5.084,15 (cost_all_on − cost_current do fixture).
    expect(await screen.findByText(/7\.028,73/)).toBeInTheDocument();
    expect(screen.getByText(/5\.084,15/)).toBeInTheDocument();
    expect(screen.getByText(/12\.112,88/)).toBeInTheDocument();
  });

  it("dinheiro de terceiros: os 5 estados epistêmicos aparecem sem número fabricado", async () => {
    mockCommands({ get_tags_screen: RICH_DTO });
    render(<TagsScreen />);
    await screen.findByText("Dinheiro de terceiros");

    expect(screen.getByText("A seu favor")).toBeInTheDocument();
    expect(screen.getByText("Em aberto há 13 dias")).toBeInTheDocument();
    expect(screen.getByText("Falta 1 parcela")).toBeInTheDocument();
    expect(screen.getByText("Quitado")).toBeInTheDocument();
    expect(screen.getByText("Sem registro")).toBeInTheDocument();
    // Bruna sem lançamento no mês: nunca um "R$ 0,00" fabricado.
    expect(screen.getByText("—")).toBeInTheDocument();
  });

  // ---------------------------------------------------------------------
  // Switches: role, aria-checked, nome acessível estável, escrita real.
  // ---------------------------------------------------------------------

  it("switch de régua: role switch, aria-checked reflete counts_in, nome estável", async () => {
    mockCommands({ get_tags_screen: RICH_DTO, update_tag_rulers_cmd: null });
    render(<TagsScreen />);
    await screen.findByText("Exceções");
    // "Gio" nomeia a TAG (exceção) e a PESSOA (terceiro) — escopado à seção de Exceções.
    const excecoes = screen.getByText("Exceções").closest("section")!;
    await userEvent.click(within(excecoes).getByText("Gio").closest("summary")!);

    const sw = screen.getByRole("switch", { name: "Performance · tag Gio" });
    expect(sw).toHaveAttribute("aria-checked", "false");

    await userEvent.click(sw);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("update_tag_rulers_cmd", {
        tagId: "gio",
        excludeFromPerformance: false,
        excludeFromCostOfLiving: true,
        excludeFromSavings: true,
        excludeFromDailyAvg: true,
      }),
    );
  });

  it("escrita em voo trava as 4 réguas da MESMA tag (lost update): o 2º clique espera", async () => {
    // O UPDATE grava as 4 colunas de uma vez; um 2º clique montado da base velha
    // desfaria o 1º em silêncio. Enquanto a escrita voa, a tag inteira trava.
    let release!: () => void;
    const pending = new Promise<null>((res) => {
      release = () => res(null);
    });
    mockCommands({
      get_tags_screen: RICH_DTO,
      update_tag_rulers_cmd: () => pending,
    });
    render(<TagsScreen />);
    await screen.findByText("Exceções");
    const excecoes = screen.getByText("Exceções").closest("section")!;
    await userEvent.click(within(excecoes).getByText("Gio").closest("summary")!);

    const perf = within(excecoes).getByRole("switch", {
      name: "Performance · tag Gio",
    });
    const custo = within(excecoes).getByRole("switch", {
      name: "Custo de vida · tag Gio",
    });
    await userEvent.click(perf);
    expect(custo).toBeDisabled();
    expect(perf).toBeDisabled();
    // Régua de OUTRA tag segue livre — o trava é por tag, não global.
    await userEvent.click(within(excecoes).getByText("Trânsito").closest("summary")!);
    expect(
      within(excecoes).getByRole("switch", { name: "Performance · tag Trânsito" }),
    ).toBeEnabled();

    release();
    await waitFor(() => expect(perf).toBeEnabled());
  });

  it("resumo da exceção: 'fora de N de 4 réguas' e o rótulo ligado a todas", async () => {
    mockCommands({ get_tags_screen: RICH_DTO });
    render(<TagsScreen />);
    await screen.findByText("Exceções");
    // Duas exceções de 4×4 (Gio, Trânsito) e uma de 1×4 (Reembolso).
    expect(screen.getAllByText(/Fora de 4 de 4 réguas/)).toHaveLength(2);
    expect(screen.getByText(/Fora de 1 de 4 réguas/)).toBeInTheDocument();
  });

  it("frase da régua: metade fixa sempre visível; efeito só quando desligada", async () => {
    mockCommands({ get_tags_screen: RICH_DTO });
    render(<TagsScreen />);
    await screen.findByText("Exceções");
    const excecoes = screen.getByText("Exceções").closest("section")!;
    const gioRow = within(excecoes).getByText("Gio").closest("details")!;
    await userEvent.click(within(gioRow).getByText("Gio").closest("summary")!);

    expect(within(gioRow).getByText(/Quanto sobrou no mês\./)).toBeInTheDocument();
    expect(within(gioRow).getByText(/entra mais do que sai/)).toBeInTheDocument();
  });

  // ---------------------------------------------------------------------
  // Estados da manchete (D–F; A/B/C específicos abaixo).
  // ---------------------------------------------------------------------

  it("estado E — carregando mostra o esqueleto (nunca um número fabricado)", () => {
    mockCommands({ get_tags_screen: RICH_DTO });
    render(<TagsScreen />);
    expect(screen.getByRole("status", { name: "Carregando" })).toBeInTheDocument();
  });

  it("estado D — zero tags ensina o conceito e oferece a CTA de criar", async () => {
    mockCommands({ get_tags_screen: dto({ tags: [] }) });
    render(<TagsScreen />);
    expect(await screen.findByText("Tags não são categorias.")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Criar primeira tag" }),
    ).toBeInTheDocument();
    expect(screen.queryByText("Exceções")).not.toBeInTheDocument();
  });

  it("estado B — sem exceção, terceiros detectados: manchete + estimativa + CTA", async () => {
    mockCommands({
      get_tags_screen: dto({
        tags: [MORADIA],
        verdict: {
          cost_current_cents: 702873,
          cost_all_on_cents: 702873,
          third_party_avg_cents: 282300,
          third_party_people: 5,
          has_exceptions: false,
        },
      }),
    });
    render(<TagsScreen />);
    expect(
      await screen.findByText("Suas réguas contam dinheiro que não é seu."),
    ).toBeInTheDocument();
    expect(screen.getByText("Estimativa")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Tirar isso das réguas" }),
    ).toBeInTheDocument();
  });

  it("estado C — sem exceção e sem detecção: número seco, sem parabéns", async () => {
    mockCommands({
      get_tags_screen: dto({
        tags: [MORADIA],
        verdict: {
          cost_current_cents: 702873,
          cost_all_on_cents: 702873,
          third_party_avg_cents: null,
          third_party_people: 0,
          has_exceptions: false,
        },
      }),
    });
    render(<TagsScreen />);
    expect(
      await screen.findByText("Nenhuma exceção declarada — e nada a declarar."),
    ).toBeInTheDocument();
  });

  it("estado F — revalidação falhou com cache: o número fica, com a idade e 'Tentar de novo'", async () => {
    const good = dto({
      tags: [MORADIA],
      last_sync_at: "2026-07-15 11:42:00",
    });
    // 1ª leitura boa; as seguintes falham — o caminho real do F é a revalidação
    // no remount encontrando o cache da última leitura boa.
    let calls = 0;
    mockCommands({
      get_tags_screen: () =>
        calls++ === 0 ? good : new Error("planilha indisponível"),
    });
    const first = render(<TagsScreen />);
    expect(await first.findByText(/7\.028,73/)).toBeInTheDocument();
    // Leitura boa NUNCA mostra a manchete F, mesmo com last_sync_at preenchido.
    expect(
      first.queryByText(/Não foi possível ler a planilha agora/),
    ).not.toBeInTheDocument();
    first.unmount();

    render(<TagsScreen />);
    expect(
      await screen.findByText(/Não foi possível ler a planilha agora/),
    ).toBeInTheDocument();
    expect(screen.getByText(/7\.028,73/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Tentar de novo" })).toBeInTheDocument();
  });

  it("erro duro sem cache: EmptyState de erro com retry (fora do A–F)", async () => {
    mockCommands({ get_tags_screen: new Error("boom") });
    render(<TagsScreen />);
    expect(
      await screen.findByText("Não foi possível carregar as tags"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Tentar novamente" }),
    ).toBeInTheDocument();
  });

  // ---------------------------------------------------------------------
  // Capacidade preservada: criar/editar tag.
  // ---------------------------------------------------------------------

  it("cria uma tag pelo botão 'Nova tag' do sec-head de Exceções", async () => {
    mockCommands({ get_tags_screen: RICH_DTO, create_tag_cmd: "new-id" });
    render(<TagsScreen />);
    await screen.findByText("Exceções");

    await userEvent.click(screen.getByRole("button", { name: "Nova tag" }));
    await userEvent.type(screen.getByLabelText("Nome da tag"), "Viagem");
    await userEvent.click(screen.getByRole("radio", { name: "Azul" }));
    await userEvent.click(screen.getByRole("button", { name: "Criar tag" }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("create_tag_cmd", {
        name: "Viagem",
        color: "var(--cat-sky)",
        emoji: null,
        isSpecial: false,
      }),
    );
  });

  it("edita uma tag existente (nome + cor) pela ação dentro da linha expandida", async () => {
    mockCommands({ get_tags_screen: RICH_DTO, update_tag_cmd: null });
    render(<TagsScreen />);
    await screen.findByText("Moradia");
    const moradiaRow = screen.getByText("Moradia").closest("details")!;
    await userEvent.click(within(moradiaRow).getByText("Moradia").closest("summary")!);

    await userEvent.click(
      within(moradiaRow).getByRole("button", { name: /Editar tag/ }),
    );
    const nameInput = screen.getByLabelText("Nome da tag");
    expect(nameInput).toHaveValue("Moradia");

    await userEvent.clear(nameInput);
    await userEvent.type(nameInput, "Casa");
    await userEvent.click(screen.getByRole("button", { name: "Salvar tag" }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("update_tag_cmd", {
        tagId: "moradia",
        name: "Casa",
        color: "var(--cat-coral)",
        emoji: null,
      }),
    );
  });
});
