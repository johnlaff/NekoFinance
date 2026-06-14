import React from "react";

const CSS = `
.nk-owner{display:inline-flex;align-items:center;gap:7px;padding:3px 10px 3px 3px;border-radius:var(--radius-pill);
  font-family:var(--font-sans);font-size:12.5px;font-weight:600;background:var(--surface-elevated);
  border:1px solid var(--border);color:var(--text);line-height:1;white-space:nowrap;}
.nk-owner__av{width:20px;height:20px;border-radius:50%;flex:none;display:inline-flex;align-items:center;
  justify-content:center;font-size:10px;font-weight:700;color:#06140E;}
.nk-owner__role{font-size:10px;font-weight:600;color:var(--text-faint);text-transform:uppercase;
  letter-spacing:.05em;padding-left:5px;margin-left:1px;border-left:1px solid var(--border);}
.nk-owner--bare{background:none;border:none;padding:3px 0;}
.nk-owner--shared .nk-owner__av{background:var(--owner-shared);}
.nk-owner--personal .nk-owner__av{background:var(--owner-personal);}
.nk-owner--partner .nk-owner__av{background:var(--owner-partner);}
.nk-owner__av--split{background:linear-gradient(135deg,var(--owner-personal) 0 50%,var(--owner-partner) 50% 100%) !important;color:#fff;}
`;

function useCSS() {
  React.useEffect(() => {
    if (document.getElementById("nk-owner-css")) return;
    const s = document.createElement("style");
    s.id = "nk-owner-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

function initials(name) {
  return name
    .split(/\s+/)
    .map((w) => w[0])
    .slice(0, 2)
    .join("")
    .toUpperCase();
}

export function OwnerChip({
  name,
  type = "personal",
  note = null,
  bare = false,
  className = "",
}) {
  useCSS();
  const isShared = type === "shared";
  return (
    <span
      className={[
        "nk-owner",
        `nk-owner--${type}`,
        bare ? "nk-owner--bare" : "",
        className,
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <span
        className={["nk-owner__av", isShared ? "nk-owner__av--split" : ""]
          .filter(Boolean)
          .join(" ")}
      >
        {isShared ? "◐" : initials(name)}
      </span>
      <span>{name}</span>
      {note ? <span className="nk-owner__role">{note}</span> : null}
    </span>
  );
}
