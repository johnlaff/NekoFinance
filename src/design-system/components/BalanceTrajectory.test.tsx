import { render } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { BalanceTrajectory } from "./BalanceTrajectory";
import { fmtAxisBRL } from "../../lib/format";
import type { ForecastDay } from "../../lib/api";

const day = (date: string, balance: number): ForecastDay => ({
  date,
  income_cents: 0,
  fixed_out_cents: 0,
  daily_out_cents: 0,
  economia_cents: 0,
  balance_cents: balance,
});

describe("fmtAxisBRL — rótulo de ponto pt-BR (mil/mi, nunca k/M)", () => {
  it("formata milhares e milhões por extenso, sem decimal, e usa minus tipográfico", () => {
    expect(fmtAxisBRL(580000)).toBe("R$ 6 mil");
    expect(fmtAxisBRL(1300000)).toBe("R$ 13 mil");
    expect(fmtAxisBRL(-32000)).toBe("−R$ 320");
    expect(fmtAxisBRL(125_000_000)).toBe("R$ 1 mi");
  });

  it("promoção pós-arredondamento nas fronteiras: nunca dois registros para a mesma magnitude", () => {
    // mil→mi: R$ 999.500 arredonda a 1.000 mil → promove ("R$ 1.000 mil" seria absurdo).
    expect(fmtAxisBRL(99_950_000)).toBe("R$ 1 mi");
    // 1 centavo abaixo do ponto de promoção fica no registro "mil".
    expect(fmtAxisBRL(99_949_999)).toBe("R$ 999 mil");
    // reais→mil: R$ 999,50 arredonda a R$ 1.000 → promove (R$ 1.000,00 exato já é "R$ 1 mil").
    expect(fmtAxisBRL(99_950)).toBe("R$ 1 mil");
    expect(fmtAxisBRL(100_000)).toBe("R$ 1 mil");
    // 1 centavo abaixo do ponto de promoção fica em reais.
    expect(fmtAxisBRL(99_949)).toBe("R$ 999");
    // Negativos promovem igual (o sinal não muda a faixa).
    expect(fmtAxisBRL(-99_950_000)).toBe("−R$ 1 mi");
    expect(fmtAxisBRL(-99_950)).toBe("−R$ 1 mil");
  });
});

describe("BalanceTrajectory", () => {
  const daily = [
    day("2026-06-10", 800000),
    day("2026-06-15", 580000),
    day("2026-06-20", -40000),
  ];

  it("renderiza a linha com stroke-draw e a banda do zero quando há déficit", () => {
    const { container } = render(
      <BalanceTrajectory daily={daily} today="2026-06-10" />,
    );
    // A linha tem a classe de stroke-draw.
    expect(container.querySelector(".nk-spark__line")).not.toBeNull();
    // Déficit → linha tracejada do zero.
    const dashed = container.querySelector('line[stroke-dasharray="3 4"]');
    expect(dashed).not.toBeNull();
    // Acessível como imagem.
    expect(container.querySelector('svg[role="img"]')).not.toBeNull();
  });

  it("sem déficit, não desenha a linha do zero", () => {
    const positive = [day("2026-06-10", 800000), day("2026-06-20", 900000)];
    const { container } = render(
      <BalanceTrajectory daily={positive} today="2026-06-10" variant="compact" />,
    );
    expect(container.querySelector('line[stroke-dasharray="3 4"]')).toBeNull();
  });
});
