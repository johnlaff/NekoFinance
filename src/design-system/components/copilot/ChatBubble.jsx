import React from "react";

const CSS = `
.nk-chat{display:flex;gap:11px;font-family:var(--font-sans);max-width:560px;}
.nk-chat--user{flex-direction:row-reverse;margin-left:auto;}
.nk-chat__av{width:30px;height:30px;flex:none;border-radius:9px;overflow:hidden;}
.nk-chat__av--user{border-radius:50%;background:var(--surface-elevated);border:1px solid var(--border);
  display:flex;align-items:center;justify-content:center;font-size:11px;font-weight:700;color:var(--text-muted);}
.nk-chat__col{display:flex;flex-direction:column;gap:5px;min-width:0;}
.nk-chat--user .nk-chat__col{align-items:flex-end;}
.nk-chat__name{font-size:11px;font-weight:600;color:var(--text-faint);display:flex;align-items:center;gap:6px;}
.nk-chat__pupil{width:7px;height:9px;border-radius:50% / 60%;background:var(--primary);display:inline-block;
  animation:nk-blink 2.4s var(--ease-calm) infinite;}
@keyframes nk-blink{0%,92%,100%{transform:scaleY(1)}96%{transform:scaleY(.15)}}
@media (prefers-reduced-motion:reduce){.nk-chat__pupil{animation:none;}}
.nk-chat__bubble{padding:10px 13px;border-radius:var(--radius-md);font-size:13.5px;line-height:1.5;color:var(--text);}
.nk-chat--mia .nk-chat__bubble{background:var(--surface);border:1px solid var(--border);border-top-left-radius:4px;}
.nk-chat--user .nk-chat__bubble{background:var(--primary-quiet);border:1px solid rgba(63,191,143,.22);
  color:var(--text-strong);border-top-right-radius:4px;}
.nk-chat__bubble p{margin:0 0 8px;}
.nk-chat__bubble p:last-child{margin:0;}
.nk-chat__bubble b{color:var(--text-strong);font-weight:700;}
.nk-chat__money{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-weight:600;}
.nk-chat__think{display:inline-flex;gap:4px;align-items:center;padding:11px 14px;}
.nk-chat__think i{width:6px;height:6px;border-radius:50%;background:var(--text-faint);
  animation:nk-bob 1.1s var(--ease-standard) infinite;}
.nk-chat__think i:nth-child(2){animation-delay:.15s;}
.nk-chat__think i:nth-child(3){animation-delay:.3s;}
@keyframes nk-bob{0%,80%,100%{opacity:.3;transform:translateY(0)}40%{opacity:1;transform:translateY(-3px)}}
@media (prefers-reduced-motion:reduce){.nk-chat__think i{animation:none;}}
`;

function useCSS() {
  React.useEffect(() => {
    if (document.getElementById("nk-chat-css")) return;
    const s = document.createElement("style");
    s.id = "nk-chat-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }, []);
}

const MIA =
  "data:image/svg+xml;utf8," +
  encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40"><rect width="40" height="40" rx="11" fill="#1F2827"/><path fill-rule="evenodd" clip-rule="evenodd" fill="#3FBF8F" d="M11 15 L9 5 L17.5 11.5 C19 11 21 11 22.5 11.5 L31 5 L29 15 C31.5 17.5 32.5 20.3 32.5 23 C32.5 29 27.5 33.5 20 33.5 C12.5 33.5 7.5 29 7.5 23 C7.5 20.3 8.5 17.5 11 15 Z M16.6 22 C16.6 23.3 15.9 24.3 15 24.3 C14.1 24.3 13.4 23.3 13.4 22 C13.4 20.7 14.1 19.7 15 19.7 C15.9 19.7 16.6 20.7 16.6 22 Z M26.6 22 C26.6 23.3 25.9 24.3 25 24.3 C24.1 24.3 23.4 23.3 23.4 22 C23.4 20.7 24.1 19.7 25 19.7 C25.9 19.7 26.6 20.7 26.6 22 Z M20 26 L18.4 24.7 C18.9 24.3 21.1 24.3 21.6 24.7 Z"/></svg>',
  );

export function ChatBubble({
  from = "mia",
  name,
  thinking = false,
  userInitials = "You",
  children,
  className = "",
}) {
  useCSS();
  const isMia = from === "mia";
  return (
    <div
      className={["nk-chat", `nk-chat--${from}`, className].filter(Boolean).join(" ")}
    >
      {isMia ? (
        <img className="nk-chat__av" src={MIA} alt="Mia" width="30" height="30" />
      ) : (
        <span className="nk-chat__av nk-chat__av--user">{userInitials}</span>
      )}
      <div className="nk-chat__col">
        {isMia ? (
          <span className="nk-chat__name">
            <span className="nk-chat__pupil" aria-hidden="true" />
            {name || "Mia"}
          </span>
        ) : null}
        {thinking ? (
          <div className="nk-chat__bubble">
            <span className="nk-chat__think">
              <i />
              <i />
              <i />
            </span>
          </div>
        ) : (
          <div className="nk-chat__bubble">{children}</div>
        )}
      </div>
    </div>
  );
}
