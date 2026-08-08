import type { ReactNode } from "react";

// ---------------------------------------------------------------------------
// Linha da gramática de card da direção: título + sub à esquerda, controle à
// direita; tile de ícone só na primeira linha de cada seção. Compartilhado entre
// SettingsScreen e os componentes de features que vivem dentro dos cards de
// Configurações (ex.: features/updater/UpdateSettingsBlock).
// ---------------------------------------------------------------------------

export function Line({
  icon: Icon,
  title,
  sub,
  subExtra,
  right,
}: {
  icon?: React.ComponentType<{ size?: number; strokeWidth?: number }>;
  title: ReactNode;
  sub: ReactNode;
  subExtra?: ReactNode;
  right?: ReactNode;
}) {
  return (
    <div className="config__line">
      {Icon ? (
        <span className="config__lineic">
          <Icon size={17} strokeWidth={1.75} />
        </span>
      ) : null}
      <div className="config__what">
        <div className="config__what-t">{title}</div>
        <div className="config__what-s">{sub}</div>
        {subExtra}
      </div>
      {right != null ? <span className="config__right">{right}</span> : null}
    </div>
  );
}

export function SecHead({
  icon: Icon,
  id,
  title,
  action,
}: {
  icon: React.ComponentType<{
    size?: number;
    strokeWidth?: number;
    className?: string;
  }>;
  id: string;
  title: string;
  action?: ReactNode;
}) {
  return (
    <header className="config__sechead">
      <Icon size={16} strokeWidth={1.75} className="ic" />
      <h2 id={id}>{title}</h2>
      {action}
    </header>
  );
}
