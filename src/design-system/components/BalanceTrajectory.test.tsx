import { render } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { BalanceTrajectory, fmtCompactBRL } from "./BalanceTrajectory";
import type { ForecastDay } from "../../lib/api";

const day = (date: string, balance: number): ForecastDay => ({
  date,
  income_cents: 0,
  fixed_out_cents: 0,
  daily_out_cents: 0,
  economia_cents: 0,
  balance_cents: balance,
});

describe("fmtCompactBRL", () => {
  it("formata milhares e usa minus tipográfico", () => {
    expect(fmtCompactBRL(580000)).toBe("R$ 5.8k");
    expect(fmtCompactBRL(1300000)).toBe("R$ 13k");
    expect(fmtCompactBRL(-32000)).toBe("−R$ 320");
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
