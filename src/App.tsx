import { useEffect, useState } from "react";
import "./App.css";
import { AppShell, type Screen } from "./shell/AppShell";
import { DashboardScreen } from "./screens/DashboardScreen";
import { TotaisScreen } from "./screens/TotaisScreen";
import { AnnualScreen } from "./screens/AnnualScreen";
import { HorizonteScreen } from "./screens/HorizonteScreen";
import { TagsScreen } from "./screens/TagsScreen";
import { TransactionsScreen } from "./screens/TransactionsScreen";
import { CopilotScreen } from "./screens/CopilotScreen";
import { MethodologyScreen } from "./screens/MethodologyScreen";
import { SettingsScreen } from "./screens/SettingsScreen";
import { OnboardingFlow, ONBOARDING_KEY } from "./features/onboarding/OnboardingFlow";
import { checkAuthStatus, getAppSetting, isTauri, type AuthStatus } from "./lib/api";

function App() {
  const [screen, setScreen] = useState<Screen>("dashboard");
  const [searchQuery, setSearchQuery] = useState("");
  const [authStatus, setAuthStatus] = useState<AuthStatus>(
    isTauri ? "loading" : "disconnected",
  );
  // `null` = ainda carregando a preferência; evita um flash do onboarding já concluído.
  // Fora do Tauri (preview web) não há onboarding.
  const [showOnboarding, setShowOnboarding] = useState<boolean | null>(
    isTauri ? null : false,
  );

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

  // React Compiler memoizes; no manual useCallback needed.
  const handleSearch = (query: string) => {
    setSearchQuery(query);
    setScreen("transactions");
  };

  return (
    <>
      {showOnboarding && (
        <OnboardingFlow
          onDone={() => setShowOnboarding(false)}
          onGoToSettings={() => setScreen("settings")}
        />
      )}
      <AppShell
        active={screen}
        onNavigate={setScreen}
        onSearch={handleSearch}
        authStatus={authStatus}
      >
        <div key={screen} className="ak-screen">
          {screen === "dashboard" && (
            <DashboardScreen onAskMia={() => setScreen("copilot")} />
          )}
          {screen === "totais" && <TotaisScreen />}
          {screen === "anuais" && <AnnualScreen />}
          {screen === "horizonte" && <HorizonteScreen />}
          {screen === "tags" && <TagsScreen />}
          {screen === "transactions" && (
            <TransactionsScreen query={searchQuery} onQueryChange={setSearchQuery} />
          )}
          {screen === "copilot" && <CopilotScreen />}
          {screen === "methodology" && <MethodologyScreen />}
          {screen === "settings" && (
            <SettingsScreen authStatus={authStatus} onAuthChange={setAuthStatus} />
          )}
        </div>
      </AppShell>
    </>
  );
}

export default App;
