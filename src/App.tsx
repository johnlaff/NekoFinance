import { useEffect, useRef, useState } from "react";
import "./App.css";
import { AppShell, type Screen } from "./shell/AppShell";
import { DashboardScreen } from "./screens/DashboardScreen";
import { TotaisScreen } from "./screens/TotaisScreen";
import { AnnualScreen } from "./screens/AnnualScreen";
import { YearGridScreen } from "./screens/YearGridScreen";
import { EconomiaCompareScreen } from "./screens/EconomiaCompareScreen";
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

  // Ponte do atalho "N": o DashboardScreen entrega o ref do campo de valor; o AppShell chama
  // handleQuickAdd ao pressionar "N". Fora do dashboard, navega e foca depois do card montar (rAF).
  const quickAddInputRef = useRef<HTMLInputElement | null>(null);

  // React Compiler memoizes; no manual useCallback needed.
  const handleSearch = (query: string) => {
    setSearchQuery(query);
    setScreen("transactions");
  };

  const handleQuickAdd = () => {
    if (screen !== "dashboard") {
      setScreen("dashboard");
      requestAnimationFrame(() => quickAddInputRef.current?.focus());
    } else {
      quickAddInputRef.current?.focus();
    }
  };

  return (
    <>
      {showOnboarding && (
        <OnboardingFlow
          onDone={() => setShowOnboarding(false)}
          onGoToSettings={() => setScreen("settings")}
        />
      )}
      {/* Enquanto o onboarding (aria-modal) está aberto, o resto fica `inert`: teclado, ponteiro e
          leitor de tela não alcançam o fundo. `display:contents` não cria caixa de layout. */}
      <div style={{ display: "contents" }} inert={showOnboarding === true}>
        <AppShell
          active={screen}
          onNavigate={setScreen}
          onSearch={handleSearch}
          authStatus={authStatus}
          onQuickAdd={handleQuickAdd}
        >
          <div key={screen} className="ak-screen">
            {screen === "dashboard" && (
              <DashboardScreen
                onAskMia={() => setScreen("copilot")}
                onQuickAddAmountRef={(el) => {
                  quickAddInputRef.current = el;
                }}
              />
            )}
            {screen === "totais" && <TotaisScreen />}
            {screen === "anuais" && <AnnualScreen />}
            {screen === "ano-inteiro" && <YearGridScreen />}
            {screen === "economia-compare" && <EconomiaCompareScreen />}
            {screen === "horizonte" && <HorizonteScreen />}
            {screen === "tags" && <TagsScreen />}
            {screen === "transactions" && (
              <TransactionsScreen
                query={searchQuery}
                onGoToSettings={() => setScreen("settings")}
              />
            )}
            {screen === "copilot" && <CopilotScreen />}
            {screen === "methodology" && <MethodologyScreen />}
            {screen === "settings" && (
              <SettingsScreen authStatus={authStatus} onAuthChange={setAuthStatus} />
            )}
          </div>
        </AppShell>
      </div>
    </>
  );
}

export default App;
