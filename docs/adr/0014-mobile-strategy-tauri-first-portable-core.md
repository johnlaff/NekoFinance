# ADR-0014: Tauri-first mobile strategy on a portable core

The app targets Android in addition to desktop. Two viable routes exist: adding the Android
target to the existing Tauri 2 project (reusing both the React UI and the Rust core), or
rebuilding the UI in React Native with the Rust core exposed through UniFFI bindings. The first
reuses everything but rides on the Android System WebView; the second buys native UI fidelity at
the cost of rewriting every screen and re-expressing the design system outside CSS.

## Decision

**Tauri Android is the primary route. React Native + UniFFI is the declared fallback, activated
exclusively by failure of the Android spike's acceptance gate** — a verifiable pass/fail
checklist exercised on real hardware (scroll performance on the densest screens, virtual-keyboard
layout integrity, animation fluidity under the design system, cold-start budget, full real-sheet
import on device), with a bounded fix budget per item. Taste on the day does not activate the
fallback; the checklist does.

To keep the fallback cheap for as long as it exists, the core stays portable by construction:

1. **No Tauri types inside domain modules** (`reading/`, `forecast/`, `mia/`, `google_sheets/`,
   `oauth/`). The shell (`commands/`, `lib.rs`) translates at the boundary. UniFFI — or any other
   host — must be able to wrap the same core without surgery.
2. **Platform capabilities enter through adapter traits** — secret storage, notifications,
   scheduling. `cfg(target_os)` selects the adapter in the shell, never inside the domain.

## Considered alternatives

- **React Native first**: rejected — it pays the port's largest possible cost (full UI rewrite,
  design-system re-expression, loss of the visual test suite) up front, to avoid a risk the spike
  can measure for the price of a few days.
- **Coupling the core to Tauri and hoping**: rejected — every leaked `tauri::` type is debt that
  compounds with each new command, and the exit option quietly rots. The boundary rule keeps the
  option priced honestly.

## Why record it here

The fallback trigger is the part a future reader would get wrong: without this record, a rough
week of WebView bugs looks like grounds to "just switch to native", discarding a working port
over friction the gate already priced in — or, symmetrically, a failed gate gets argued down
because sunk cost. The strategy is two-legged on purpose, and the legs are not interchangeable:
the gate, not sentiment, moves work from one to the other.
