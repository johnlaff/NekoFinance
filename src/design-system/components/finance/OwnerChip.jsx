import React from "react";

// OwnerChip — who owns a transaction/account (personal / partner / shared).
// Inline-style only; no CSS injection. Matches production OwnerChip.tsx exactly.

const OWNERS = {
  personal: { label: "Eu", color: "var(--owner-personal)" },
  partner: { label: "Parceiro(a)", color: "var(--owner-partner)" },
  shared: { label: "Compartilhado", color: "var(--owner-shared)" },
};

const CHIP_BASE = {
  display: "inline-flex",
  alignItems: "center",
  borderRadius: "var(--radius-pill)",
  fontSize: "var(--fs-micro)",
  color: "var(--text-muted)",
  whiteSpace: "nowrap",
  fontFamily: "var(--font-sans)",
};

const AVATAR_BASE = {
  width: 20,
  height: 20,
  borderRadius: "50%",
  flex: "none",
  display: "inline-grid",
  placeItems: "center",
  background: "var(--surface-elevated)",
  color: "var(--text)",
  fontSize: "var(--fs-label)",
  fontWeight: "var(--fw-bold)",
};

function initials(name) {
  return name
    .trim()
    .split(/\s+/)
    .map((w) => w[0] ?? "")
    .slice(0, 2)
    .join("")
    .toUpperCase();
}

export function OwnerChip({
  who = "personal",
  name,
  note,
  bare = false,
  avatar = false,
  className = "",
}) {
  const o = OWNERS[who] || OWNERS.personal;
  const label = name != null ? name : o.label;

  const chipStyle = {
    ...CHIP_BASE,
    gap: avatar ? "7px" : "6px",
    height: avatar ? 26 : 22,
    padding: bare ? 0 : avatar ? "0 10px 0 3px" : "0 9px 0 7px",
    border: bare ? "none" : "var(--bw-hair) solid var(--border)",
    background: bare ? "none" : "var(--surface-2)",
  };

  const avatarStyle = {
    ...AVATAR_BASE,
    border: `var(--bw-strong) solid ${o.color}`,
  };

  return (
    <span
      className={className}
      title={note ? `${label} · ${note}` : label}
      style={chipStyle}
    >
      {avatar ? (
        <span aria-hidden="true" style={avatarStyle}>
          {initials(label)}
        </span>
      ) : (
        <span
          aria-hidden="true"
          style={{
            width: 7,
            height: 7,
            borderRadius: "50%",
            flex: "none",
            background: o.color,
          }}
        />
      )}
      {label}
      {note ? <span style={{ color: "var(--text-faint)" }}>{note}</span> : null}
    </span>
  );
}
