/* Neko Finance — App shell: sidebar + topbar + optional copilot dock.
   Composes DS Badge/Button. Exposes window.AppShell. */
(function () {
  const NS = window.NekoFinanceDesignSystem_9bd1cd;
  const { Badge, Button } = NS;
  const Icon = window.Icon;

  const CSS = `
  .ak{display:grid;grid-template-columns:var(--sidebar-w) 1fr;height:100%;background:var(--bg);color:var(--text);
    font-family:var(--font-sans);}
  .ak--dock{grid-template-columns:var(--sidebar-w) 1fr var(--copilot-w);}
  /* sidebar */
  .ak-side{display:flex;flex-direction:column;border-right:1px solid var(--border);background:var(--bg-subtle);min-height:0;}
  .ak-brand{display:flex;align-items:center;gap:10px;padding:16px 18px 14px;}
  .ak-brand__mark{width:26px;height:26px;color:var(--primary);}
  .ak-brand__name{font-size:18px;font-weight:700;letter-spacing:-0.02em;color:var(--text-strong);}
  .ak-brand__tag{margin-left:auto;font-size:9px;font-weight:700;letter-spacing:.07em;text-transform:uppercase;
    color:var(--text-faint);display:flex;align-items:center;gap:4px;}
  .ak-nav{display:flex;flex-direction:column;gap:2px;padding:6px 10px;flex:1;}
  .ak-navh{font-size:10px;font-weight:700;letter-spacing:.08em;text-transform:uppercase;color:var(--text-faint);
    padding:14px 10px 6px;}
  .ak-item{display:flex;align-items:center;gap:11px;padding:8px 10px;border-radius:var(--radius-sm);
    font-size:13.5px;font-weight:500;color:var(--text-muted);cursor:pointer;border:none;background:none;
    text-align:left;width:100%;transition:var(--t-hover);position:relative;}
  .ak-item:hover{background:var(--surface-hover);color:var(--text);}
  .ak-item--active{background:var(--surface-selected);color:var(--text-strong);font-weight:600;}
  .ak-item--active::before{content:"";position:absolute;left:-10px;top:7px;bottom:7px;width:2px;border-radius:2px;background:var(--primary);}
  .ak-item__ic{color:inherit;opacity:.85;flex:none;}
  .ak-item__badge{margin-left:auto;}
  .ak-side__foot{padding:12px;border-top:1px solid var(--border);display:flex;flex-direction:column;gap:10px;}
  .ak-conn{display:flex;align-items:center;gap:9px;padding:9px 11px;background:var(--surface);border:1px solid var(--border);
    border-radius:var(--radius-sm);}
  .ak-conn__ic{width:26px;height:26px;border-radius:7px;background:var(--success-tint);color:var(--success-400);
    display:flex;align-items:center;justify-content:center;flex:none;}
  .ak-conn__t{font-size:11.5px;font-weight:600;color:var(--text);}
  .ak-conn__s{font-size:10.5px;color:var(--text-faint);}
  /* main */
  .ak-main{display:flex;flex-direction:column;min-width:0;min-height:0;}
  .ak-top{display:flex;align-items:center;gap:14px;height:var(--topbar-h);padding:0 22px;border-bottom:1px solid var(--border);
    background:color-mix(in srgb, var(--bg) 80%, transparent);backdrop-filter:blur(8px);flex:none;}
  .ak-top__titles{flex:none;min-width:0;}
  .ak-top__title{font-size:16px;font-weight:700;color:var(--text-strong);letter-spacing:-0.01em;white-space:nowrap;line-height:1.15;}
  .ak-top__crumb{font-size:12px;color:var(--text-faint);white-space:nowrap;line-height:1.2;}
  .ak-search{display:flex;align-items:center;gap:8px;height:32px;padding:0 11px;background:var(--surface);
    border:1px solid var(--border);border-radius:var(--radius-sm);color:var(--text-faint);width:230px;font-size:12.5px;}
  .ak-search input{background:none;border:none;outline:none;color:var(--text);font-family:inherit;font-size:12.5px;width:100%;}
  .ak-spacer{flex:1;}
  .ak-iconbtn{width:32px;height:32px;border-radius:var(--radius-sm);border:1px solid var(--border);background:var(--surface);
    color:var(--text-muted);display:flex;align-items:center;justify-content:center;cursor:pointer;transition:var(--t-hover);}
  .ak-iconbtn:hover{background:var(--surface-hover);color:var(--text);}
  .ak-body{flex:1;overflow:auto;padding:22px;min-height:0;}
  .ak-body--flush{padding:0;overflow:hidden;}
  /* dock */
  .ak-dock{border-left:1px solid var(--border);background:var(--bg-subtle);min-height:0;display:flex;flex-direction:column;}
  `;

  function ensureCSS() {
    if (document.getElementById("ak-css")) return;
    const s = document.createElement("style");
    s.id = "ak-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }

  const NAV = [
    { key: "dashboard", label: "Dashboard", icon: "dashboard" },
    { key: "transactions", label: "Transactions", icon: "receipt", badge: 6 },
    { key: "copilot", label: "Ask Mia", icon: "sparkles" },
    { key: "methodology", label: "Methodology", icon: "book" },
  ];

  function Mark() {
    return React.createElement("span", {
      className: "ak-brand__mark",
      dangerouslySetInnerHTML: {
        __html:
          '<svg viewBox="0 0 48 48" fill="none"><path fill="currentColor" fill-rule="evenodd" clip-rule="evenodd" d="M12 17 L9.2 5.4 L20 13.2 C22 12.6 26 12.6 28 13.2 L38.8 5.4 L36 17 C39.4 20 40.5 23.5 40.5 27 C40.5 35 33.5 41.5 24 41.5 C14.5 41.5 7.5 35 7.5 27 C7.5 23.5 8.6 20 12 17 Z M18.5 25.5 C18.5 27.2 17.6 28.5 16.4 28.5 C15.2 28.5 14.3 27.2 14.3 25.5 C14.3 23.8 15.2 22.5 16.4 22.5 C17.6 22.5 18.5 23.8 18.5 25.5 Z M33.7 25.5 C33.7 27.2 32.8 28.5 31.6 28.5 C30.4 28.5 29.5 27.2 29.5 25.5 C29.5 23.8 30.4 22.5 31.6 22.5 C32.8 22.5 33.7 23.8 33.7 25.5 Z M24 30.2 L22 28.6 C22.6 28.1 25.4 28.1 26 28.6 Z"/></svg>',
      },
    });
  }

  function ThemeToggle() {
    const get = () =>
      typeof document !== "undefined" &&
      document.documentElement.getAttribute("data-theme") === "light"
        ? "light"
        : "dark";
    const [theme, setTheme] = React.useState(get());
    React.useEffect(() => {
      window.__nekoThemeListeners = window.__nekoThemeListeners || [];
      window.__nekoThemeListeners.push(setTheme);
      return () => {
        window.__nekoThemeListeners = (window.__nekoThemeListeners || []).filter(
          (f) => f !== setTheme,
        );
      };
    }, []);
    const toggle = () => {
      const root = document.documentElement;
      const next = theme === "light" ? "dark" : "light";
      root.setAttribute("data-theme", next);
      try {
        localStorage.setItem("neko-theme", next);
      } catch (e) {}
      (window.__nekoThemeListeners || []).forEach((f) => f(next));
    };
    return React.createElement(
      "button",
      {
        className: "ak-iconbtn",
        title: "Toggle theme",
        "aria-label": "Toggle light or dark theme",
        onClick: toggle,
      },
      React.createElement(Icon, { name: theme === "light" ? "moon" : "sun", size: 17 }),
    );
  }

  function AppShell({
    active = "dashboard",
    onNav = () => {},
    title,
    crumb,
    right,
    children,
    dock = null,
    flush = false,
  }) {
    ensureCSS();
    return React.createElement(
      "div",
      { className: "ak" + (dock ? " ak--dock" : "") },
      // sidebar
      React.createElement(
        "aside",
        { className: "ak-side" },
        React.createElement(
          "div",
          { className: "ak-brand" },
          React.createElement(Mark),
          React.createElement("span", { className: "ak-brand__name" }, "Neko"),
          React.createElement(
            "span",
            { className: "ak-brand__tag" },
            React.createElement(Icon, { name: "lock", size: 11 }),
            "Local",
          ),
        ),
        React.createElement(
          "nav",
          { className: "ak-nav" },
          React.createElement("div", { className: "ak-navh" }, "Finances"),
          ...NAV.map((n) =>
            React.createElement(
              "button",
              {
                key: n.key,
                className: "ak-item" + (active === n.key ? " ak-item--active" : ""),
                onClick: () => onNav(n.key),
              },
              React.createElement(Icon, {
                name: n.icon,
                size: 18,
                className: "ak-item__ic",
              }),
              React.createElement("span", null, n.label),
              n.badge
                ? React.createElement(
                    "span",
                    { className: "ak-item__badge" },
                    React.createElement(
                      Badge,
                      { tone: "warning", square: true },
                      n.badge,
                    ),
                  )
                : null,
            ),
          ),
          React.createElement("div", { className: "ak-navh" }, "System"),
          React.createElement(
            "button",
            {
              className: "ak-item" + (active === "settings" ? " ak-item--active" : ""),
              onClick: () => onNav("settings"),
            },
            React.createElement(Icon, {
              name: "settings",
              size: 18,
              className: "ak-item__ic",
            }),
            React.createElement("span", null, "Settings & privacy"),
          ),
        ),
        React.createElement(
          "div",
          { className: "ak-side__foot" },
          React.createElement(
            "div",
            { className: "ak-conn" },
            React.createElement(
              "span",
              { className: "ak-conn__ic" },
              React.createElement(Icon, { name: "table", size: 15 }),
            ),
            React.createElement(
              "div",
              null,
              React.createElement(
                "div",
                { className: "ak-conn__t" },
                "Sheets connected",
              ),
              React.createElement(
                "div",
                { className: "ak-conn__s" },
                "Synced 2m ago · read-only",
              ),
            ),
          ),
        ),
      ),
      // main
      React.createElement(
        "div",
        { className: "ak-main" },
        React.createElement(
          "header",
          { className: "ak-top" },
          React.createElement(
            "div",
            { className: "ak-top__titles" },
            React.createElement("div", { className: "ak-top__title" }, title),
            crumb
              ? React.createElement("div", { className: "ak-top__crumb" }, crumb)
              : null,
          ),
          React.createElement("div", { className: "ak-spacer" }),
          React.createElement(
            "label",
            { className: "ak-search" },
            React.createElement(Icon, { name: "search", size: 15 }),
            React.createElement("input", {
              placeholder: "Search transactions, rules…",
            }),
          ),
          right || null,
          React.createElement(ThemeToggle),
          React.createElement(
            "button",
            { className: "ak-iconbtn" },
            React.createElement(Icon, { name: "bell", size: 17 }),
          ),
        ),
        React.createElement(
          "div",
          { className: "ak-body" + (flush ? " ak-body--flush" : "") },
          children,
        ),
      ),
      // dock
      dock ? React.createElement("aside", { className: "ak-dock" }, dock) : null,
    );
  }

  window.AppShell = AppShell;
})();
