import { useEffect, useState } from "react";
import { FileUp, HardDrive, Link2, type LucideIcon } from "lucide-react";
import { GoogleSheetsPanel } from "../features/sheets/GoogleSheetsPanel";
import { LocalXlsxImport } from "../features/sheets/LocalXlsxImport";
import { getAppInfo, isTauri, type AppInfo, type AuthStatus } from "../lib/api";

function Section({
  icon: Icon,
  title,
  sub,
  children,
}: {
  icon: LucideIcon;
  title: string;
  sub?: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <div className="set-sec__head">
        <h2 className="set-sec__title">
          <Icon size={17} strokeWidth={1.75} className="set-sec__ic" />
          {title}
        </h2>
        {sub ? <div className="set-sec__sub">{sub}</div> : null}
      </div>
      {children}
    </section>
  );
}

export function SettingsScreen({
  authStatus,
  onAuthChange,
}: {
  authStatus: AuthStatus;
  onAuthChange: (status: AuthStatus) => void;
}) {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    if (!isTauri) return;
    getAppInfo()
      .then(setAppInfo)
      .catch(() => setAppInfo(null));
  }, []);

  return (
    <div className="set">
      <Section
        icon={Link2}
        title="Conexão Google Sheets"
        sub="O Neko lê sua planilha. Nada é escrito sem a sua aprovação."
      >
        <div className="set-panel set-panel--pad">
          <GoogleSheetsPanel authStatus={authStatus} onAuthChange={onAuthChange} />
        </div>
      </Section>

      <Section
        icon={FileUp}
        title="Importar arquivo local"
        sub="Use uma cópia .xlsx da planilha quando não quiser conectar a conta Google."
      >
        <div className="set-panel">
          <div className="set-row">
            <div className="set-row__main">
              <div className="set-row__t">Planilha .xlsx</div>
              <div className="set-row__d">
                Importa todas as abas, detectando o layout de blocos mensais
                automaticamente. Linhas já importadas são ignoradas (dedup).
              </div>
            </div>
            <div className="set-row__ctl">
              <LocalXlsxImport />
            </div>
          </div>
        </div>
      </Section>

      <Section
        icon={HardDrive}
        title="Seus dados"
        sub="O Neko é local-first: não existe conta Neko nem backend."
      >
        <div className="set-panel">
          <div className="set-row">
            <div className="set-row__main">
              <div className="set-row__t">Onde ficam os dados</div>
              <div className="set-row__d">
                Banco SQLite em <code>{appInfo ? appInfo.db_path : "—"}</code>, somente
                neste dispositivo.
              </div>
            </div>
          </div>
          <div className="set-row">
            <div className="set-row__main">
              <div className="set-row__t">Telemetria</div>
              <div className="set-row__d">
                O Neko não envia nenhum dado de uso. Suas finanças não saem da sua
                máquina.
              </div>
            </div>
          </div>
          <div className="set-row">
            <div className="set-row__main">
              <div className="set-row__t">Versão</div>
              <div className="set-row__d">
                Neko Finance {appInfo ? `v${appInfo.version}` : "—"} · Tauri desktop
              </div>
            </div>
          </div>
        </div>
      </Section>
    </div>
  );
}
