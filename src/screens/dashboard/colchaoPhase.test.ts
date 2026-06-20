import { describe, expect, it } from "vitest";
import type { DashboardSummary, Forecast } from "../../lib/api";
import { colchaoPhase, RESERVE_MIN_MONTHS } from "./colchaoPhase";

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

  it("calibrates at 3 months even when rate is met — below method floor", () => {
    expect(
      colchaoPhase(
        { ...summary, reserve_months: 3 },
        {
          ...forecast,
          annual_savings: {
            ...forecast.annual_savings,
            registered_economia_cents: 200_000,
          },
        },
      ),
    ).toBe("calibrate");
  });

  it("calibrates at 5 months — at-risk zone, not yet at floor", () => {
    expect(
      colchaoPhase(
        { ...summary, reserve_months: 5 },
        {
          ...forecast,
          annual_savings: {
            ...forecast.annual_savings,
            registered_economia_cents: 200_000,
          },
        },
      ),
    ).toBe("calibrate");
  });

  it("operates at exactly RESERVE_MIN_MONTHS when rate is met", () => {
    expect(
      colchaoPhase(
        { ...summary, reserve_months: RESERVE_MIN_MONTHS },
        {
          ...forecast,
          annual_savings: {
            ...forecast.annual_savings,
            registered_economia_cents: 200_000,
          },
        },
      ),
    ).toBe("operate");
  });
});
