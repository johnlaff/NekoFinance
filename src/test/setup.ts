import "@testing-library/jest-dom/vitest";

// Mock Tauri environment for tests
Object.defineProperty(window, "__TAURI_INTERNALS__", {
  value: {},
  writable: true,
  configurable: true,
});

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
