import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { PerformanceCard } from "./PerformanceCard";
import { FORECAST, EMPTY_FORECAST } from "../../test/commands";

// Characterization tests (plan 010): props-only card. PIN how upcoming-month
// performance cells render today (labels, rate %, incomplete marker, null case).

describe("PerformanceCard", () => {
  it("renders_performance_for_upcoming_months", () => {
    // FORECAST.today = "2026-06-10" → ym "2026-06"; meses 6 e 7 passam o filtro.
    render(<PerformanceCard forecast={FORECAST} />);
    // monthNamePtBR é minúsculo: "junho"/"julho".
    expect(screen.getByText("junho")).toBeInTheDocument();
    expect(screen.getByText("julho")).toBeInTheDocument();
    // savings_rate_bps: junho 2500 → 25%, julho 1000 → 10%.
    expect(screen.getByText(/economizado 25%/)).toBeInTheDocument();
    expect(screen.getByText(/economizado 10%/)).toBeInTheDocument();
  });

  it("marks_incomplete_month_with_incompleto_label", () => {
    // FORECAST.coverage já marca agosto/2026 incompleto; adicionamos agosto em months.
    const withIncomplete = {
      ...FORECAST,
      months: [
        ...FORECAST.months,
        {
          year: 2026,
          month: 8,
          income_cents: 500000,
          performance_cents: 100000,
          cost_of_living_cents: 400000,
          fixed_out_cents: 400000,
          daily_out_cents: 0,
          real_daily_avg_cents: 0,
          economia_cents: 0,
          savings_rate_bps: 2000,
        },
      ],
    };
    render(<PerformanceCard forecast={withIncomplete} />);
    expect(screen.getByText("incompleto")).toBeInTheDocument();
  });

  it("returns_null_when_no_upcoming_months", () => {
    // EMPTY_FORECAST.months = [] → o componente não renderiza nada.
    const { container } = render(<PerformanceCard forecast={EMPTY_FORECAST} />);
    expect(container.firstChild).toBeNull();
  });
});
