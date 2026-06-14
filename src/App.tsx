import { useEffect, useState } from "react";
import "./App.css";
import { AppShell, type Screen } from "./shell/AppShell";
import { DashboardScreen } from "./screens/DashboardScreen";
import { TotaisScreen } from "./screens/TotaisScreen";
import { TransactionsScreen } from "./screens/TransactionsScreen";
import { CopilotScreen } from "./screens/CopilotScreen";
import { MethodologyScreen } from "./screens/MethodologyScreen";
import { SettingsScreen } from "./screens/SettingsScreen";
import { checkAuthStatus, isTauri, type AuthStatus } from "./lib/api";

function App() {
  const [screen, setScreen] = useState<Screen>("dashboard");
  const [searchQuery, setSearchQuery] = useState("");
  const [authStatus, setAuthStatus] = useState<AuthStatus>(
    isTauri ? "loading" : "disconnected",
  );

  useEffect(() => {
    if (!isTauri) return;
    checkAuthStatus()
      .then(setAuthStatus)
      .catch(() => setAuthStatus("disconnected"));
  }, []);

  // React Compiler memoizes; no manual useCallback needed.
  const handleSearch = (query: string) => {
    setSearchQuery(query);
    setScreen("transactions");
  };

  return (
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
  );
}

export default App;
