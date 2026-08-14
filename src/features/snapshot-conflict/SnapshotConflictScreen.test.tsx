import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mockCommands, mockInvoke } from "../../test/commands";
import {
  gestureKeys,
  type DriveConflictDetails,
  type DriveConflictGesture,
} from "./snapshotConflictView";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { SnapshotConflictScreen } from "./SnapshotConflictScreen";
import {
  closeSnapshotConflict,
  openSnapshotConflict,
  snapshotConflictOpenSnapshot,
} from "./snapshotConflictStore";

const DETAILS: DriveConflictDetails = {
  remote_manifest: {
    device_id: "abcdef12-3456-7890-abcd-ef1234567890",
    sequence: 5,
    created_at: "2026-08-12T08:00:00Z",
    app_version: "0.2.1",
    schema_version: 1,
  },
  local_gestures: [
    {
      at: "2026-08-11 10:00:00",
      event_type: "import",
      entity_type: "transaction",
      source_sheet: "Diário",
    },
    {
      at: "2026-08-12 07:00:00",
      event_type: "write_back",
      entity_type: "transaction",
      source_sheet: "Saídas",
    },
  ],
  remote_gestures: [
    {
      at: "2026-08-12 06:00:00",
      event_type: "import",
      entity_type: "transaction",
      source_sheet: "Cartão",
    },
  ],
  this_device_id: "este-aparelho-11111111",
};

describe("SnapshotConflictScreen", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    closeSnapshotConflict();
  });

  it("carrega e mostra os gestos de CADA lado antes de qualquer escolha", async () => {
    mockCommands({ drive_conflict_details: DETAILS });
    render(<SnapshotConflictScreen />);

    expect(
      await screen.findByText(/Importação da planilha \(aba Diário\)/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Escrita de volta na planilha \(aba Saídas\)/),
    ).toBeInTheDocument();
    // O gesto do OUTRO aparelho aparece numa lista própria — o que se perde se o dono escolher
    // manter este aparelho, nunca misturado com os gestos locais.
    expect(
      screen.getByText(/Importação da planilha \(aba Cartão\)/),
    ).toBeInTheDocument();
    expect(screen.getAllByText(/outro aparelho \(abcdef12\)/).length).toBeGreaterThan(
      0,
    );
    expect(
      screen.getByRole("button", { name: "Manter este aparelho" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Usar o outro aparelho" }),
    ).toBeInTheDocument();
  });

  it("gestureKeys: a BASE (antes do sufixo de ocorrência) nunca colide entre gestos distintos", () => {
    function gesture(overrides: Partial<DriveConflictGesture>): DriveConflictGesture {
      return {
        at: "2026-08-12 09:00:00",
        event_type: "import",
        entity_type: "transaction",
        source_sheet: null,
        ...overrides,
      };
    }

    // `source_sheet` é dado do usuário (nome de aba digitado por ele) e pode conter "|" — com
    // `join("|")` isto colide com um outro gesto cujos campos, concatenados, formam o MESMO
    // literal ("...|transaction|Extra|1" nos dois casos abaixo). Essa colisão de BASE faz `b`
    // cair no ramo de "segunda ocorrência da base de `a`" — sua chave final vira `${keyA}|1`,
    // o MESMO sufixo que um gesto DUPLICADO de `a` receberia: a tela não teria como distinguir
    // "gesto novo que colidiu" de "duplicata genuína". Checar só `keyA !== keyB` não pega isto —
    // o sufixo de ocorrência sempre torna as duas chaves diferentes como STRING mesmo quando a
    // base colide por baixo (a asserção antiga passava com `join("|")`, vácua).
    const a = gesture({ source_sheet: "Extra|1" });
    const b = gesture({ entity_type: "transaction|Extra", source_sheet: "1" });
    const [keyA, keyB] = gestureKeys([a, b]);

    expect(keyB).not.toBe(`${keyA}|1`);
  });

  it("a copy declara o recorte real das listas e a origem do relógio do outro lado", async () => {
    mockCommands({ drive_conflict_details: DETAILS });
    render(<SnapshotConflictScreen />);

    // Regra 7 do ui-standards: a copy só afirma o que o dado confirma — o `sync_log` hoje só
    // registra import/write-back da planilha, nunca split/tag/reembolso/fatura/teto/cenário.
    // `findByText` (não `findByRole("status")`): o `EmptyState` de carregamento TAMBÉM usa
    // `role="status"` — a busca por texto evita pegar o status errado numa corrida com o fetch.
    await screen.findByText(/As listas abaixo cobrem só importações/);
    const status = screen.getByText(/^Isto é/);
    expect(status).toHaveTextContent(
      "As listas abaixo cobrem só importações e escritas na planilha",
    );
    expect(status).toHaveTextContent(
      "split, tag, reembolso, fatura, teto e cenário ainda não ficam registrados aqui",
    );
    // Os horários da lista remota vêm do relógio do OUTRO aparelho — nunca lidos como se
    // tivessem passado pela sincronização deste.
    expect(status).toHaveTextContent(
      "Os horários do lado do outro aparelho vêm do relógio dele, não deste",
    );
  });

  it("mostra o estado honesto quando um dos lados não tem gesto nenhum registrado", async () => {
    mockCommands({
      drive_conflict_details: { ...DETAILS, local_gestures: [], remote_gestures: [] },
    });
    render(<SnapshotConflictScreen />);

    expect(
      await screen.findByText(
        /Não há registro de importação ou escrita na planilha neste aparelho/,
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        /Não há registro de importação ou escrita na planilha no outro aparelho/,
      ),
    ).toBeInTheDocument();
  });

  it("mostra erro de carregamento com saída para fechar, sem travar a tela", async () => {
    const user = userEvent.setup();
    mockCommands({ drive_conflict_details: new Error("rede fora do ar") });
    render(<SnapshotConflictScreen />);

    expect(
      await screen.findByText("Não foi possível carregar o conflito"),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Fechar" }));
    expect(snapshotConflictOpenSnapshot()).toBe(false);
  });

  it("manter este aparelho: publica e fecha a tela sem exigir reinício", async () => {
    const user = userEvent.setup();
    mockCommands({
      drive_conflict_details: DETAILS,
      resolve_drive_conflict: {
        choice: "keep_local",
        requires_restart: false,
        sequence: 6,
      },
    });
    render(<SnapshotConflictScreen />);

    await user.click(
      await screen.findByRole("button", { name: "Manter este aparelho" }),
    );

    await waitFor(() => expect(snapshotConflictOpenSnapshot()).toBe(false));
    const call = mockInvoke.mock.calls.find((c) => c[0] === "resolve_drive_conflict");
    // A sequência do manifest que a TELA mostrou (5, `DETAILS.remote_manifest.sequence`) viaja
    // no gesto — é o que sustenta a recusa por consentimento obsoleto do lado do backend.
    expect(call?.[1]).toMatchObject({ choice: "keep_local", seenRemoteSequence: 5 });
  });

  it("usar o outro aparelho: exige reinício e nunca fecha a tela sozinha", async () => {
    const user = userEvent.setup();
    openSnapshotConflict();
    mockCommands({
      drive_conflict_details: DETAILS,
      resolve_drive_conflict: {
        choice: "use_remote",
        requires_restart: true,
        sequence: 5,
      },
    });
    render(<SnapshotConflictScreen />);

    await user.click(
      await screen.findByRole("button", { name: "Usar o outro aparelho" }),
    );

    expect(
      await screen.findByText("Feche e abra o Neko Finance de novo"),
    ).toBeInTheDocument();
    // Trocar o arquivo ativo debaixo do pool em uso exige reiniciar — a tela nunca finge que o
    // app segue operável fechando sozinha.
    expect(snapshotConflictOpenSnapshot()).toBe(true);
    const call = mockInvoke.mock.calls.find((c) => c[0] === "resolve_drive_conflict");
    expect(call?.[1]).toMatchObject({ choice: "use_remote", seenRemoteSequence: 5 });
  });

  it("mostra verbatim um erro atrás do prefixo de contrato de restauração", async () => {
    const user = userEvent.setup();
    openSnapshotConflict();
    mockCommands({
      drive_conflict_details: DETAILS,
      resolve_drive_conflict: new Error(
        "Restauração recusada: o snapshot do outro aparelho foi publicado por uma versão " +
          "mais nova do Neko Finance (schema 9 > 7) — atualize o app antes de continuar.",
      ),
    });
    render(<SnapshotConflictScreen />);

    await user.click(
      await screen.findByRole("button", { name: "Usar o outro aparelho" }),
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Restauração recusada: o snapshot do outro aparelho foi publicado por uma versão mais nova",
    );
    expect(snapshotConflictOpenSnapshot()).toBe(true);
    expect(
      screen.getByRole("button", { name: "Manter este aparelho" }),
    ).not.toBeDisabled();
  });

  it("um erro sem prefixo de contrato cai no fallback calmo, nunca vaza texto técnico cru", async () => {
    const user = userEvent.setup();
    openSnapshotConflict();
    mockCommands({
      drive_conflict_details: DETAILS,
      resolve_drive_conflict: new Error(
        "error returned from database: (code: 5) database is locked",
      ),
    });
    render(<SnapshotConflictScreen />);

    await user.click(
      await screen.findByRole("button", { name: "Manter este aparelho" }),
    );

    const alert = await screen.findByRole("alert");
    expect(alert).not.toHaveTextContent("database is locked");
    expect(alert).toHaveTextContent("banco local está ocupado");
  });

  it("consentimento obsoleto: recarrega os detalhes em vez de mostrar um erro parado", async () => {
    const user = userEvent.setup();
    openSnapshotConflict();
    const UPDATED_DETAILS: DriveConflictDetails = {
      ...DETAILS,
      remote_manifest: { ...DETAILS.remote_manifest, sequence: 7 },
      remote_gestures: [
        {
          at: "2026-08-12 08:30:00",
          event_type: "import",
          entity_type: "transaction",
          source_sheet: "Cartão",
        },
      ],
    };
    let detailsCallCount = 0;
    mockCommands({
      drive_conflict_details: () => {
        detailsCallCount += 1;
        return detailsCallCount === 1 ? DETAILS : UPDATED_DETAILS;
      },
      resolve_drive_conflict: new Error(
        "Check-in recusado: a disputa mudou de novo desde que você abriu esta tela — veja " +
          "os detalhes atualizados antes de escolher.",
      ),
    });
    render(<SnapshotConflictScreen />);

    await user.click(
      await screen.findByRole("button", { name: "Manter este aparelho" }),
    );

    // A tela busca os detalhes DE NOVO — nunca mostra a recusa como um erro parado — e o dono vê
    // o estado atualizado (o outro aparelho já em outra sequência) para decidir de novo.
    await waitFor(() => expect(detailsCallCount).toBe(2));
    expect(
      await screen.findByRole("button", { name: "Manter este aparelho" }),
    ).toBeEnabled();
    // O silêncio vira uma nota visível e calma — o dono nunca deve achar que o clique não fez
    // nada: ele viu um spinner e a tela voltou com as listas do outro aparelho já atualizadas.
    expect(
      screen.getByText(
        "A disputa mudou desde que esta tela abriu — os detalhes abaixo já são os atualizados.",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(snapshotConflictOpenSnapshot()).toBe(true);
  });

  it("erro pós-fechamento do pool: trava em reiniciar, nunca reoferece os botões de escolha", async () => {
    const user = userEvent.setup();
    openSnapshotConflict();
    mockCommands({
      drive_conflict_details: DETAILS,
      resolve_drive_conflict: new Error(
        "trocar pelo snapshot baixado: Os arquivos de origem são diferentes; reinicie o app " +
          "para continuar",
      ),
    });
    render(<SnapshotConflictScreen />);

    await user.click(
      await screen.findByRole("button", { name: "Usar o outro aparelho" }),
    );

    expect(await screen.findByText("Reinicie o Neko Finance")).toBeInTheDocument();
    // Nenhum botão de escolha sobrevive — não há pool para uma nova tentativa nesta sessão.
    expect(
      screen.queryByRole("button", { name: "Manter este aparelho" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Usar o outro aparelho" }),
    ).not.toBeInTheDocument();
    expect(snapshotConflictOpenSnapshot()).toBe(true);
  });

  it("Decidir depois fecha a tela sem publicar nem restaurar nada", async () => {
    const user = userEvent.setup();
    openSnapshotConflict();
    mockCommands({ drive_conflict_details: DETAILS });
    render(<SnapshotConflictScreen />);

    await user.click(await screen.findByRole("button", { name: "Decidir depois" }));

    expect(snapshotConflictOpenSnapshot()).toBe(false);
    expect(mockInvoke.mock.calls.some((c) => c[0] === "resolve_drive_conflict")).toBe(
      false,
    );
  });
});
