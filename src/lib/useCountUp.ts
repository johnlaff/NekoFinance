import { useEffect, useRef, useState } from "react";

const DURATION_MS = 480; // --dur-deliberate
// --ease-entrance flattened to a cheap approximation for rAF interpolation
const easeOut = (t: number) => 1 - Math.pow(1 - t, 4);

/** Last value each counter actually displayed, so re-navigations don't replay the count. */
const lastShown = new Map<string, number>();

function prefersInstant(): boolean {
  return (
    typeof window === "undefined" ||
    typeof window.matchMedia !== "function" ||
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/**
 * Counts from the last displayed value (0 on the first appearance this
 * session) to `target` over the design system's deliberate duration. Snaps
 * instantly under reduced motion or in environments without matchMedia
 * (jsdom), keeping tests deterministic.
 */
export function useCountUp(target: number, key = "default"): number {
  const initial = prefersInstant() ? target : (lastShown.get(key) ?? 0);
  const [value, setValue] = useState(initial);
  const fromRef = useRef(initial);

  useEffect(() => {
    if (prefersInstant() || fromRef.current === target) {
      fromRef.current = target;
      lastShown.set(key, target);
      setValue(target);
      return;
    }
    const from = fromRef.current;
    fromRef.current = target;
    lastShown.set(key, target);
    const start = performance.now();
    let raf = 0;
    const tick = (now: number) => {
      const t = Math.min((now - start) / DURATION_MS, 1);
      setValue(Math.round(from + (target - from) * easeOut(t)));
      if (t < 1) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [target, key]);

  return value;
}
