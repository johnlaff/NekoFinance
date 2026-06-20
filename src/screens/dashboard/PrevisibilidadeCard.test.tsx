import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { PrevisibilidadeCard } from "./PrevisibilidadeCard";
import { FORECAST, EMPTY_FORECAST } from "../../test/commands";

// Characterization tests (plan 010): props-only card, no Tauri calls. These PIN the
// current rendered states of the previsibilidade card so plan 011 has a safety net.

describe("PrevisibilidadeCard", () => {
  it("renders_incomplete_month_warning", () => {
    // FORECAST tem um mês incompleto (agosto/2026) e baseline > 0.
    render(<PrevisibilidadeCard forecast={FORECAST} />);
    // Parágrafo de alerta "A partir de <mês> faltam ...".
    expect(screen.getByText(/A partir de/)).toBeInTheDocument();
    // Economizado% = round(250000 / 5000000 * 100) = 5.
    expect(screen.getByText("5%")).toBeInTheDocument();
  });

  it("renders_neutral_when_no_baseline", () => {
    // EMPTY_FORECAST tem baseline_outflow_cents = 0 → estado neutro.
    render(<PrevisibilidadeCard forecast={EMPTY_FORECAST} />);
    expect(screen.getByText(/Ainda não há meses realizados/)).toBeInTheDocument();
    // Sem o parágrafo de alerta de meses incompletos.
    expect(screen.queryByText(/A partir de/)).not.toBeInTheDocument();
  });

  it("renders_ok_when_all_months_complete", () => {
    const complete = {
      ...FORECAST,
      trusted_through_month: "2026-07",
      coverage: FORECAST.coverage.map((c) => ({ ...c, is_complete: true })),
    };
    render(<PrevisibilidadeCard forecast={complete} />);
    expect(screen.getByText(/Seus meses futuros estão completos/)).toBeInTheDocument();
  });

  it("renders_trusted_through_label", () => {
    // FORECAST.trusted_through_month = "2026-07" → "confiável até julho".
    render(<PrevisibilidadeCard forecast={FORECAST} />);
    expect(screen.getByText(/confiável até/)).toBeInTheDocument();
  });
});
