import { useEffect, useMemo, useState } from "react";
import "./App.css";
import "./redesign.css";
import { AppShell, type Screen } from "./shell/AppShell";
import { NekoAppProvider, type ComposeOptions } from "./shell/appContext";
import { Compose } from "./shell/Compose";
import { DashboardScreen } from "./screens/DashboardScreen";
import { TransactionsScreen } from "./screens/TransactionsScreen";
import { TotaisScreen } from "./screens/TotaisScreen";
import { AnnualScreen } from "./screens/AnnualScreen";
import { YearGridScreen } from "./screens/YearGridScreen";
import { HorizonteScreen } from "./screens/HorizonteScreen";
import { TagsScreen } from "./screens/TagsScreen";
import { CopilotScreen } from "./screens/CopilotScreen";
import { SettingsScreen } from "./screens/SettingsScreen";
import { OnboardingFlow, ONBOARDING_KEY } from "./features/onboarding/OnboardingFlow";
import {
  checkAuthStatus,
  getAppSetting,
  getForecast,
  isTauri,
  type AuthStatus,
} from "./lib/api";
import { useCommand } from "./lib/useCommand";
import { fmtCompact } from "./lib/nkFormat";

function App() {
  const [screen, setScreen] = useState<Screen>("hoje");
  const [authStatus, setAuthStatus] = useState<AuthStatus>(
    isTauri ? "loading" : "disconnected",
  );
  const [showOnboarding, setShowOnboarding] = useState<boolean | null>(
    isTauri ? null : false,
  );

  // Estado do compositor de lançamento (drawer "Lançar"). `seq` remonta o Compose a cada abertura.
  const [compose, setCompose] = useState<{
    open: boolean;
    options: ComposeOptions;
    seq: number;
  }>({
    open: false,
    options: {},
    seq: 0,
  });

  useEffect(() => {
    if (!isTauri) return;
    checkAuthStatus()
      .then(setAuthStatus)
      .catch(() => setAuthStatus("disconnected"));
  }, []);

  useEffect(() => {
    if (!isTauri) return;
    getAppSetting(ONBOARDING_KEY)
      .then((v) => setShowOnboarding(v !== "true"))
      .catch(() => setShowOnboarding(false));
  }, []);

  // Dicas numéricas da nav (saldo de hoje, performance do mês). Reusam o cache compartilhado.
  const forecastQ = useCommand("get_forecast", getForecast);
  const hints = useMemo<Partial<Record<Screen, string>>>(() => {
    const out: Partial<Record<Screen, string>> = {};
    const f = forecastQ.data;
    if (f) out.hoje = fmtCompact(Math.max(0, f.safe_to_spend_today_cents));
    if (f) {
      const ym = f.today.slice(0, 7);
      const mm = f.months.find(
        (m) => `${m.year}-${String(m.month).padStart(2, "0")}` === ym,
      );
      if (mm) out.mes = fmtCompact(mm.performance_cents);
    }
    return out;
  }, [forecastQ.data]);

  const nekoApp = useMemo(
    () => ({
      navigate: (s: Screen) => setScreen(s),
      openCompose: (options: ComposeOptions = {}) =>
        setCompose((c) => ({ open: true, options, seq: c.seq + 1 })),
    }),
    [],
  );

  return (
    <NekoAppProvider value={nekoApp}>
      {showOnboarding && (
        <OnboardingFlow
          onDone={() => setShowOnboarding(false)}
          onGoToSettings={() => setScreen("config")}
        />
      )}
      <div style={{ display: "contents" }} inert={showOnboarding === true}>
        <AppShell
          active={screen}
          onNavigate={setScreen}
          authStatus={authStatus}
          onCompose={() =>
            setCompose((c) => ({
              open: true,
              options: { mode: "new" },
              seq: c.seq + 1,
            }))
          }
          hints={hints}
        >
          <div key={screen} className="ak-screen neko-app">
            {screen === "hoje" && <DashboardScreen />}
            {screen === "lancamentos" && <TransactionsScreen />}
            {screen === "mes" && <TotaisScreen />}
            {screen === "ano" && <AnnualScreen />}
            {screen === "calendario" && <YearGridScreen />}
            {screen === "horizonte" && <HorizonteScreen />}
            {screen === "tags" && <TagsScreen />}
            {screen === "mia" && <CopilotScreen />}
            {screen === "config" && (
              <SettingsScreen authStatus={authStatus} onAuthChange={setAuthStatus} />
            )}
          </div>
        </AppShell>
        <Compose
          key={compose.seq}
          open={compose.open}
          options={compose.options}
          onClose={() => setCompose((c) => ({ ...c, open: false }))}
          onSaved={() => {
            /* dados já invalidados dentro do Compose; o próximo render rebusca */
          }}
        />
      </div>
    </NekoAppProvider>
  );
}

export default App;
