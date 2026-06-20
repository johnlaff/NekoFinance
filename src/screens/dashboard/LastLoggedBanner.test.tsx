import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import type * as FormatModule from "../../lib/format";
import { LastLoggedBanner } from "./LastLoggedBanner";

// Pin "today" so tests are deterministic (todayISO() reads the real clock).
vi.mock("../../lib/format", async (importOriginal) => {
  const actual = await importOriginal<typeof FormatModule>();
  return { ...actual, todayISO: () => "2026-06-20" };
});

describe("LastLoggedBanner", () => {
  it("shows nothing when logged today (diffDays = 0)", () => {
    const { container } = render(<LastLoggedBanner lastRealTxDate="2026-06-20" />);
    expect(container.firstChild).toBeNull();
  });

  it("shows 'ontem' for diffDays = 1", () => {
    render(<LastLoggedBanner lastRealTxDate="2026-06-19" />);
    expect(screen.getByRole("status")).toHaveTextContent("ontem");
  });

  it("shows the day count for diffDays > 1", () => {
    render(<LastLoggedBanner lastRealTxDate="2026-06-15" />);
    expect(screen.getByRole("status")).toHaveTextContent("há 5 dias");
  });

  it("shows a first-entry prompt when lastRealTxDate is null", () => {
    render(<LastLoggedBanner lastRealTxDate={null} />);
    expect(screen.getByRole("status")).toHaveTextContent("Nenhum lançamento ainda");
  });
});
