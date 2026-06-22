import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { TagsScreen } from "./TagsScreen";
import type { TagTotal } from "../lib/api";
import { mockCommands, mockInvoke } from "../test/commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const TOTALS: TagTotal[] = [
  {
    id: "t1",
    name: "Moradia",
    color: "var(--cat-jade)",
    emoji: null,
    is_special: false,
    exclude_from_totals: false,
    total_cents: 226070,
  },
  {
    id: "t2",
    name: "Cartão",
    color: "var(--cat-violet)",
    emoji: null,
    is_special: false,
    exclude_from_totals: false,
    total_cents: 218500,
  },
];

describe("TagsScreen (Tags)", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("renders the per-tag spend list for the month", async () => {
    mockCommands({ tag_totals_for_month_cmd: TOTALS });
    render(<TagsScreen />);
    expect(await screen.findByText("Gasto por tag")).toBeInTheDocument();
    expect(screen.getByText("Moradia")).toBeInTheDocument();
  });
});
