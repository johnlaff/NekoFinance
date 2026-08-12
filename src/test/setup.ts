import "@testing-library/jest-dom/vitest";

// Mock Tauri environment for tests
Object.defineProperty(window, "__TAURI_INTERNALS__", {
  value: {},
  writable: true,
  configurable: true,
});

// `@tauri-apps/plugin-os` reads this global synchronously (no `invoke` round trip) — real Tauri
// injects it before any frontend JS runs, and `lib/env.ts` guards on its presence before calling
// `platform()`. Desktop is the default here; `vi.mock("../../lib/env", ...)` is how individual
// tests fake Android.
Object.defineProperty(window, "__TAURI_OS_PLUGIN_INTERNALS__", {
  value: { platform: "linux" },
  writable: true,
  configurable: true,
});

// jsdom doesn't implement ResizeObserver (the compare charts measure their container to
// draw 1:1 pixels). No-op stub: components fall back to their default width in tests;
// real measurement is covered by the Playwright visual specs.
if (typeof globalThis.ResizeObserver === "undefined") {
  const noop = () => undefined;
  class ResizeObserverStub {
    observe = noop;
    unobserve = noop;
    disconnect = noop;
  }
  globalThis.ResizeObserver = ResizeObserverStub;
}

// jsdom doesn't implement IntersectionObserver (the shell's large-title coordination
// watches the screen's hero). No-op stub: unit tests assert the BIND contract; real
// visibility tracking is covered by the Playwright visual specs.
if (typeof globalThis.IntersectionObserver === "undefined") {
  const noop = () => undefined;
  class IntersectionObserverStub {
    observe = noop;
    unobserve = noop;
    disconnect = noop;
    takeRecords = () => [];
  }
  globalThis.IntersectionObserver =
    IntersectionObserverStub as unknown as typeof IntersectionObserver;
}

// jsdom doesn't implement the native <dialog> API (showModal/show/close).
// The Compose drawer uses a real <dialog>; polyfill the methods so component
// tests can open/close it. Real browsers (and the Playwright e2e) use the
// native implementation.
if (typeof HTMLDialogElement !== "undefined") {
  const proto = HTMLDialogElement.prototype;
  if (!proto.showModal) {
    proto.showModal = function showModal() {
      this.open = true;
    };
  }
  if (!proto.show) {
    proto.show = function show() {
      this.open = true;
    };
  }
  if (!proto.close) {
    proto.close = function close(returnValue?: string) {
      this.open = false;
      if (returnValue !== undefined) this.returnValue = returnValue;
      this.dispatchEvent(new Event("close"));
    };
  }
}
