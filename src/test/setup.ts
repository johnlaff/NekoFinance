import "@testing-library/jest-dom/vitest";

// Mock Tauri environment for tests
Object.defineProperty(window, "__TAURI_INTERNALS__", {
  value: {},
  writable: true,
  configurable: true,
});
