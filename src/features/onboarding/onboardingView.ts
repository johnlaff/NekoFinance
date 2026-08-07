//! Porta do domínio Onboarding: `OnboardingFlow.tsx` importa só daqui — nunca de `lib/api`.

import { setAppSetting } from "../../lib/api";

/** Marca o onboarding como concluído (gravação best-effort; falha não bloqueia o usuário). */
export function markOnboardingDone(): Promise<void> {
  return setAppSetting(ONBOARDING_KEY, "true");
}

export const ONBOARDING_KEY = "onboarding_done";
