import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useCountUp } from "./useCountUp";

describe("useCountUp", () => {
  it("snaps to the target instantly in environments without matchMedia (jsdom)", () => {
    const { result } = renderHook(() => useCountUp(842000, "test-a"));
    expect(result.current).toBe(842000);
  });

  it("tracks target changes without intermediate stale values", () => {
    const { result, rerender } = renderHook(({ v }) => useCountUp(v, "test-b"), {
      initialProps: { v: 100 },
    });
    expect(result.current).toBe(100);
    rerender({ v: 250 });
    expect(result.current).toBe(250);
  });
});
