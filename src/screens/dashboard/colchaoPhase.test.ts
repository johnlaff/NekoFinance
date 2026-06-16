import { describe, expect, it } from "vitest";
import type { DashboardSummary, Forecast } from "../../lib/api";
import { colchaoPhase } from "./colchaoPhase";

const summary = {
  transaction_count: 50,
  reserve_months: 6,
} as DashboardSummary;

const forecast = {
  annual_savings: {
    realized_income_cents: 1_000_000,
    realized_savings_cents: 400_000,
    realized_rate_bps: 4000,
    registered_economia_cents: 0,
  },
} as Forecast;

describe("colchaoPhase", () => {
  it("does not operate from net surplus when registered Economia is below 20%", () => {
    expect(colchaoPhase(summary, forecast)).toBe("calibrate");
  });

  it("operates when registered Economia reaches 20% and reserve is ready", () => {
    expect(
      colchaoPhase(summary, {
        ...forecast,
        annual_savings: {
          ...forecast.annual_savings,
          registered_economia_cents: 200_000,
        },
      }),
    ).toBe("operate");
  });
});
