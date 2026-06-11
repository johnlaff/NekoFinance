# Tasks: Motion & Interaction Polish

- [x] T1 Motion layer in App.css on DS tokens only (screen-enter, tile stagger via
      token-derived delays, deficit emphasis, nav indicator, tile hover lift,
      view-transition reset, reduced-motion guards)
- [x] T2 Circular reveal theme switch: View Transitions + WAAPI clip-path from the
      interaction point; fallbacks for missing API and reduced motion (tested)
- [x] T3 `useCommand` SWR-lite cache (tests: load, cached remount, stale-while-error,
      invalidation); wired into Dashboard/Transações/Configurações; imports call
      `invalidateCommands()`
- [x] T4 `useCountUp` hero count-up with session memory (snaps in jsdom/reduced motion — tested)
- [x] T5 ⌘K focuses header search + kbd hint (e2e)
- [x] T6 e2e: View Transitions path test, ⌘K test; screenshots refreshed
- [x] T7 `npm run check` green; commit + push
- [x] T8 App icon: Neko mark (jade on graphite tile) replacing the create-tauri-app
      defaults — `icons/icon-source.svg` + `npx tauri icon`; embedded icon group
      verified inside the built exe (wrestool/icotool extraction)
- [x] T9 Startup fluidity: window backgroundColor matches --ink-900 (kills the white
      flash); React Compiler enabled (auto-memoization, stable v1) with redundant
      manual useCallback/useMemo removed per React Doctor advisory
