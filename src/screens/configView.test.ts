import { describe, it, expect } from "vitest";
import {
  CHECKIN_REFUSED_CONFLICT,
  CHECKIN_REFUSED_PREFIX,
  CHECKIN_REFUSED_PULL,
  driveCheckinErrorMessage,
  driveCheckinLabel,
  driveCheckoutLabel,
  driveCheckoutOutcomeWarning,
  greetState,
} from "./configView";

describe("greetState — pílula de estado do veredito", () => {
  it("verificando conexão", () => {
    expect(greetState("loading", 0, 0, null)).toEqual({
      tone: "muted",
      headline: "Verificando conexão…",
      detail: null,
    });
  });

  it("desconectado sem pendência", () => {
    expect(greetState("disconnected", 0, 0, null)).toEqual({
      tone: "warn",
      headline: "Desconectado",
      detail: null,
    });
  });

  it("desconectado com mudanças aguardando (a má notícia tem o mesmo peso)", () => {
    expect(greetState("disconnected", 3, 0, null)).toEqual({
      tone: "warn",
      headline: "Desconectado",
      detail: "3 mudanças aguardando",
    });
  });

  it("sessão expirada", () => {
    expect(greetState("expired", 0, 0, "há 2 h")).toEqual({
      tone: "warn",
      headline: "Sessão expirada",
      detail: null,
    });
  });

  it("sessão expirada com pendência", () => {
    expect(greetState("expired", 1, 0, null)).toEqual({
      tone: "warn",
      headline: "Sessão expirada",
      detail: "1 mudança aguardando",
    });
  });

  it("conectado com conflito bloqueando (pior estado ganha do pending)", () => {
    expect(greetState("connected", 5, 1, "há 2 min")).toEqual({
      tone: "warn",
      headline: "Conectado",
      detail: "Conflito de importação a resolver",
    });
  });

  it("conectado com conflitos (plural)", () => {
    expect(greetState("connected", 0, 2, null)).toEqual({
      tone: "warn",
      headline: "Conectado",
      detail: "2 conflitos de importação a resolver",
    });
  });

  it("conectado com mudanças aguardando", () => {
    expect(greetState("connected", 2, 0, "há 5 min")).toEqual({
      tone: "ok",
      headline: "Conectado",
      detail: "2 mudanças aguardando",
    });
  });

  it("conectado com 1 mudança (singular)", () => {
    expect(greetState("connected", 1, 0, null)).toEqual({
      tone: "ok",
      headline: "Conectado",
      detail: "1 mudança aguardando",
    });
  });

  it("conectado e sincronizado", () => {
    expect(greetState("connected", 0, 0, "há 2 min")).toEqual({
      tone: "ok",
      headline: "Conectado",
      detail: "Sincronizado há 2 min",
    });
  });

  it("conectado sem timestamp de sync", () => {
    expect(greetState("connected", 0, 0, null)).toEqual({
      tone: "ok",
      headline: "Conectado",
      detail: null,
    });
  });
});

describe("driveCheckinLabel — recência + aparelho do último check-in do snapshot", () => {
  const now = new Date("2026-08-11T15:00:00Z").getTime();

  it("nenhum check-in ainda", () => {
    expect(driveCheckinLabel(null, now)).toBe(
      "Nenhum check-in ainda — publique o primeiro snapshot.",
    );
    expect(driveCheckinLabel(undefined, now)).toBe(
      "Nenhum check-in ainda — publique o primeiro snapshot.",
    );
  });

  it("check-in feito por este aparelho", () => {
    // `last_checkin_at` vem de `chrono::Utc::now().to_rfc3339()` (snapshot_cmds.rs) — RFC3339
    // com offset explícito, nunca o formato "YYYY-MM-DD HH:MM:SS" do sync_log.
    const label = driveCheckinLabel(
      {
        last_checkin_at: "2026-08-11T14:55:00+00:00",
        last_checkin_device_id: "device-a",
        last_checkout_at: null,
        last_checkout_device_id: null,
        last_checkout_outcome: null,
        last_checkout_outcome_detail: null,
        this_device_id: "device-a",
      },
      now,
    );
    expect(label).toBe("Último check-in há 5 min, por este aparelho.");
  });

  it("check-in feito por OUTRO aparelho (identifica pelo id curto)", () => {
    const label = driveCheckinLabel(
      {
        last_checkin_at: "2026-08-11T13:00:00+00:00",
        last_checkin_device_id: "device-bbbbbbbb-cccc",
        last_checkout_at: null,
        last_checkout_device_id: null,
        last_checkout_outcome: null,
        last_checkout_outcome_detail: null,
        this_device_id: "device-a",
      },
      now,
    );
    expect(label).toBe("Último check-in há 2 h, por outro aparelho (device-b).");
  });
});

describe("driveCheckoutLabel — recência + aparelho de origem do último check-out", () => {
  const now = new Date("2026-08-11T15:00:00Z").getTime();

  it("nenhuma leitura ainda", () => {
    expect(driveCheckoutLabel(null, now)).toBe("Nenhuma leitura do Drive ainda.");
    expect(driveCheckoutLabel(undefined, now)).toBe("Nenhuma leitura do Drive ainda.");
  });

  it("mostra de qual aparelho veio o snapshot puxado", () => {
    const label = driveCheckoutLabel(
      {
        last_checkin_at: null,
        last_checkin_device_id: null,
        last_checkout_at: "2026-08-11T14:55:00+00:00",
        last_checkout_device_id: "device-bbbbbbbb-cccc",
        last_checkout_outcome: null,
        last_checkout_outcome_detail: null,
        this_device_id: "device-a",
      },
      now,
    );
    expect(label).toBe("Última leitura há 5 min, de outro aparelho (device-b).");
  });

  it("compara com this_device_id, como o lado do check-in — nunca crava 'outro aparelho'", () => {
    // O check-out normalmente vem de OUTRO aparelho, mas o backend pode registrar o NOSSO
    // device_id ali (ex.: um estado antigo, ou um bug que este PR corrige do lado do backend) —
    // a tela não pode mentir "outro aparelho" quando o id bate com o deste aparelho.
    const label = driveCheckoutLabel(
      {
        last_checkin_at: null,
        last_checkin_device_id: null,
        last_checkout_at: "2026-08-11T14:55:00+00:00",
        last_checkout_device_id: "device-a",
        last_checkout_outcome: null,
        last_checkout_outcome_detail: null,
        this_device_id: "device-a",
      },
      now,
    );
    expect(label).toBe("Última leitura há 5 min, deste aparelho.");
  });
});

describe("driveCheckoutOutcomeWarning — aviso do desfecho do check-out (spec 043 US11)", () => {
  it("nada a avisar quando não há info ou o desfecho é null", () => {
    expect(driveCheckoutOutcomeWarning(null)).toBeNull();
    expect(driveCheckoutOutcomeWarning(undefined)).toBeNull();
    expect(
      driveCheckoutOutcomeWarning({
        last_checkin_at: null,
        last_checkin_device_id: null,
        last_checkout_at: null,
        last_checkout_device_id: null,
        last_checkout_outcome: null,
        last_checkout_outcome_detail: null,
        this_device_id: "device-a",
      }),
    ).toBeNull();
  });

  it("schema mais novo: orienta a atualizar o app", () => {
    const warning = driveCheckoutOutcomeWarning({
      last_checkin_at: null,
      last_checkin_device_id: null,
      last_checkout_at: null,
      last_checkout_device_id: null,
      last_checkout_outcome: "refused_newer_schema",
      last_checkout_outcome_detail: "3:4",
      this_device_id: "device-a",
    });
    expect(warning).toContain("atualiz");
  });

  it("falha de rede/integridade: diz que a leitura não aconteceu e que tenta na próxima abertura", () => {
    const warning = driveCheckoutOutcomeWarning({
      last_checkin_at: null,
      last_checkin_device_id: null,
      last_checkout_at: null,
      last_checkout_device_id: null,
      last_checkout_outcome: "error",
      last_checkout_outcome_detail: "timeout de rede",
      this_device_id: "device-a",
    });
    expect(warning).toContain("não aconteceu");
    expect(warning).toContain("próxima abertura");
  });
});

describe("driveCheckinErrorMessage — recusa do lease", () => {
  it("reconhece a recusa por PREFIXO estrutural — não por regex sobre as palavras da frase", () => {
    // Uma frase descritiva nunca antes vista, mas com o prefixo do contrato: reconhecida.
    const futureCopy = `${CHECKIN_REFUSED_PREFIX}uma explicação nova, ainda não escrita hoje.`;
    expect(driveCheckinErrorMessage(new Error(futureCopy))).toBe(futureCopy);
  });

  it("Pull: mostra a mensagem verbatim", () => {
    expect(driveCheckinErrorMessage(new Error(CHECKIN_REFUSED_PULL))).toBe(
      CHECKIN_REFUSED_PULL,
    );
  });

  it("Conflict: mostra a mensagem verbatim", () => {
    expect(driveCheckinErrorMessage(new Error(CHECKIN_REFUSED_CONFLICT))).toBe(
      CHECKIN_REFUSED_CONFLICT,
    );
  });

  it("erro sem o prefixo do contrato cai no fallback genérico", () => {
    expect(driveCheckinErrorMessage(new Error("falha de rede qualquer"))).toBe(
      "Não foi possível fazer o check-in.",
    );
  });
});
