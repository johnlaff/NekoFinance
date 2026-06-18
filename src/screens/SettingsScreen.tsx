import { useState } from "react";
import { FileUp, HardDrive, Landmark, Link2, type LucideIcon } from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { PocketsManager } from "../features/pockets/PocketsManager";
import { GoogleSheetsPanel } from "../features/sheets/GoogleSheetsPanel";
import { LocalXlsxImport } from "../features/sheets/LocalXlsxImport";
import { backupDatabase, getAppInfo, isTauri, type AuthStatus } from "../lib/api";
import { safeErrorMessage } from "../lib/errors";
import { useCommand } from "../lib/useCommand";
import { Button } from "../design-system/components/Button";

/** Backup local do banco: escolhe o destino no save dialog nativo e grava via VACUUM INTO. */
function DataBackupRow() {
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  // Sem `finally` de propósito: o React Compiler não otimiza componentes com try/finally.
  async function doBackup() {
    setMsg(null);
    setErr(null);
    let dest: string | null;
    try {
      dest = await save({
        title: "Salvar backup do Neko",
        defaultPath: "neko-finance-backup.db",
        filters: [{ name: "Banco SQLite", extensions: ["db"] }],
      });
    } catch (e) {
      setErr(safeErrorMessage(e, "Não foi possível abrir o seletor de arquivo."));
      return;
    }
    if (!dest) return; // usuário cancelou
    setBusy(true);
    try {
      await backupDatabase(dest);
      setBusy(false);
      setMsg("Backup salvo.");
    } catch (e) {
      setBusy(false);
      setErr(safeErrorMessage(e, "Não foi possível fazer o backup."));
    }
  }

  return (
    <div className="set-row">
      <div className="set-row__main">
        <div className="set-row__t">Backup do banco</div>
        <div className="set-row__d">
          Salva uma cópia íntegra (.db) onde você escolher — leve para outro disco ou
          dispositivo. {msg ? <strong>{msg}</strong> : null}
          {err ? (
            <strong role="alert" style={{ color: "var(--danger-400)" }}>
              {err}
            </strong>
          ) : null}
        </div>
      </div>
      <div className="set-row__ctl">
        <Button
          variant="secondary"
          size="sm"
          onClick={() => void doBackup()}
          disabled={busy || !isTauri}
        >
          {busy ? "Salvando…" : "Fazer backup"}
        </Button>
      </div>
    </div>
  );
}

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
  const appInfo = useCommand("get_app_info", getAppInfo).data ?? null;

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
                automaticamente. Linhas já importadas antes são ignoradas.
              </div>
            </div>
            <div className="set-row__ctl">
              <LocalXlsxImport />
            </div>
          </div>
        </div>
      </Section>

      <Section
        icon={Landmark}
        title="Bolsos"
        sub="Conta, poupança, vale, previdência e FGTS: só dinheiro líquido entra no saldo projetado."
      >
        <PocketsManager />
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
          <DataBackupRow />
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
