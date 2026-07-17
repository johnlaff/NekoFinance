/* @ds-bundle: {"format":3,"namespace":"NekoFinanceDesignSystem_9bd1cd","components":[{"name":"ChatBubble","sourcePath":"components/copilot/ChatBubble.jsx"},{"name":"Citation","sourcePath":"components/copilot/Citation.jsx"},{"name":"EmptyState","sourcePath":"components/copilot/EmptyState.jsx"},{"name":"MiaAvatar","sourcePath":"components/copilot/MiaAvatar.jsx"},{"name":"Badge","sourcePath":"components/core/Badge.jsx"},{"name":"Button","sourcePath":"components/core/Button.jsx"},{"name":"Disclosure","sourcePath":"components/core/Disclosure.jsx"},{"name":"InfoPopover","sourcePath":"components/core/InfoPopover.jsx"},{"name":"Input","sourcePath":"components/core/Input.jsx"},{"name":"MonthNav","sourcePath":"components/core/MonthNav.jsx"},{"name":"NekoMark","sourcePath":"components/core/NekoMark.jsx"},{"name":"SegmentedControl","sourcePath":"components/core/SegmentedControl.jsx"},{"name":"Switch","sourcePath":"components/core/Switch.jsx"},{"name":"ApprovalDiffCard","sourcePath":"components/finance/ApprovalDiffCard.jsx"},{"name":"BalanceTrajectory","sourcePath":"components/finance/BalanceTrajectory.jsx"},{"name":"HealthBadge","sourcePath":"components/finance/HealthBadge.jsx"},{"name":"LineItemEditor","sourcePath":"components/finance/LineItemEditor.jsx"},{"name":"MetricTile","sourcePath":"components/finance/MetricTile.jsx"},{"name":"Money","sourcePath":"components/finance/Money.jsx"},{"name":"MovBadge","sourcePath":"components/finance/MovBadge.jsx"},{"name":"OwnerChip","sourcePath":"components/finance/OwnerChip.jsx"},{"name":"PhaseBadge","sourcePath":"components/finance/PhaseBadge.jsx"},{"name":"ProvBadge","sourcePath":"components/finance/ProvBadge.jsx"},{"name":"TransactionRow","sourcePath":"components/finance/TransactionRow.jsx"}],"sourceHashes":{"components/copilot/ChatBubble.jsx":"167278e05c28","components/copilot/Citation.jsx":"3acb7522d21e","components/copilot/EmptyState.jsx":"c96fc443c82d","components/copilot/MiaAvatar.jsx":"2a33e3ee1d4f","components/core/Badge.jsx":"513efe284ad9","components/core/Button.jsx":"b2a4cc4d68dd","components/core/Disclosure.jsx":"26b539348e91","components/core/InfoPopover.jsx":"a9cd2a1b7494","components/core/Input.jsx":"fdae11d4cf42","components/core/MonthNav.jsx":"2801bfa6a15c","components/core/NekoMark.jsx":"ba4316e16fe2","components/core/SegmentedControl.jsx":"ff30d2741399","components/core/Switch.jsx":"741ae8f79d75","components/finance/ApprovalDiffCard.jsx":"e8d57746a455","components/finance/BalanceTrajectory.jsx":"e50400f86b78","components/finance/HealthBadge.jsx":"0992a5b38f00","components/finance/LineItemEditor.jsx":"e456bc27f716","components/finance/MetricTile.jsx":"78de6b02c8bb","components/finance/Money.jsx":"c7af88943030","components/finance/MovBadge.jsx":"82c42171008a","components/finance/OwnerChip.jsx":"d6891dfdfda9","components/finance/PhaseBadge.jsx":"b035d5f427f5","components/finance/ProvBadge.jsx":"0035a5740a63","components/finance/TransactionRow.jsx":"c066cda74b91","ui_kits/ano-inteiro/YearGridScreen.jsx":"02225ca5f063","ui_kits/anuais/AnnualScreen.jsx":"4acbc71f6d97","ui_kits/copilot/CopilotScreen.jsx":"93cc123d17e2","ui_kits/dashboard/AppShell.jsx":"ce55f7804039","ui_kits/dashboard/DashboardScreen.jsx":"d6dcfd6fe057","ui_kits/economia-compare/EconomiaCompareScreen.jsx":"520881fdb17e","ui_kits/horizonte/HorizonteScreen.jsx":"37be8563ecf1","ui_kits/methodology/MethodologyScreen.jsx":"57e26b193b40","ui_kits/settings/SettingsScreen.jsx":"f01798e6b3c3","ui_kits/shared/icons.jsx":"521d5c4cdde0","ui_kits/tags/TagsScreen.jsx":"01bc44f579a1","ui_kits/totais/TotaisScreen.jsx":"472375a5da79","ui_kits/transactions/TransactionsScreen.jsx":"0cdb4ffd302a"},"inlinedExternals":[],"unexposedExports":[]} */

(() => {
  const __ds_ns = (window.NekoFinanceDesignSystem_9bd1cd =
    window.NekoFinanceDesignSystem_9bd1cd || {});

  const __ds_scope = {};

  __ds_ns.__errors = __ds_ns.__errors || [];

  // components/copilot/ChatBubble.jsx
  try {
    (() => {
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
      function ChatBubble({
        from = "mia",
        name,
        thinking = false,
        userInitials = "You",
        children,
        className = "",
      }) {
        useCSS();
        const isMia = from === "mia";
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: ["nk-chat", `nk-chat--${from}`, className]
              .filter(Boolean)
              .join(" "),
          },
          isMia
            ? /*#__PURE__*/ React.createElement("img", {
                className: "nk-chat__av",
                src: MIA,
                alt: "Mia",
                width: "30",
                height: "30",
              })
            : /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "nk-chat__av nk-chat__av--user",
                },
                userInitials,
              ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "nk-chat__col",
            },
            isMia
              ? /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "nk-chat__name",
                  },
                  /*#__PURE__*/ React.createElement("span", {
                    className: "nk-chat__pupil",
                    "aria-hidden": "true",
                  }),
                  name || "Mia",
                )
              : null,
            thinking
              ? /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "nk-chat__bubble",
                  },
                  /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      className: "nk-chat__think",
                    },
                    /*#__PURE__*/ React.createElement("i", null),
                    /*#__PURE__*/ React.createElement("i", null),
                    /*#__PURE__*/ React.createElement("i", null),
                  ),
                )
              : /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "nk-chat__bubble",
                  },
                  children,
                ),
          ),
        );
      }
      Object.assign(__ds_scope, { ChatBubble });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/copilot/ChatBubble.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/copilot/Citation.jsx
  try {
    (() => {
      function _extends() {
        return (
          (_extends = Object.assign
            ? Object.assign.bind()
            : function (n) {
                for (var e = 1; e < arguments.length; e++) {
                  var t = arguments[e];
                  for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]);
                }
                return n;
              }),
          _extends.apply(null, arguments)
        );
      }
      const CSS = `
      .nk-cite{display:inline-flex;align-items:center;gap:5px;height:18px;padding:0 6px 0 5px;border-radius:var(--radius-xs);
        background:var(--surface-elevated);border:1px solid var(--border);font-family:var(--font-money);
        font-size:10.5px;color:var(--text-muted);vertical-align:middle;cursor:default;transition:var(--t-hover);}
      .nk-cite:hover{border-color:var(--border-strong);color:var(--text);}
      .nk-cite__n{display:inline-flex;align-items:center;justify-content:center;min-width:13px;height:13px;padding:0 3px;
        border-radius:3px;background:var(--primary-quiet);color:var(--primary);font-weight:700;font-size:9px;}
      .nk-tool{border:1px solid var(--border);border-radius:var(--radius-sm);overflow:hidden;background:var(--bg-subtle);
        font-family:var(--font-sans);max-width:420px;}
      .nk-tool__bar{display:flex;align-items:center;gap:7px;padding:7px 11px;background:var(--surface);
        border-bottom:1px solid var(--border);}
      .nk-tool__badge{font-size:9px;font-weight:700;letter-spacing:.06em;text-transform:uppercase;color:var(--primary);
        background:var(--primary-quiet);padding:2px 6px;border-radius:3px;}
      .nk-tool__fn{font-family:var(--font-money);font-size:11.5px;color:var(--text);font-weight:500;}
      .nk-tool__body{padding:9px 11px;display:flex;flex-direction:column;gap:5px;}
      .nk-tool__line{display:flex;justify-content:space-between;gap:12px;font-size:12px;}
      .nk-tool__line span:first-child{color:var(--text-muted);}
      .nk-tool__line span:last-child{font-family:var(--font-money);font-variant-numeric:tabular-nums;color:var(--text);}
      .nk-tool__total{border-top:1px solid var(--border);margin-top:3px;padding-top:7px;font-weight:700;}
      .nk-tool__total span:last-child{color:var(--primary);font-weight:700;}
      .nk-tool__src{display:flex;align-items:center;gap:6px;padding:7px 11px;border-top:1px solid var(--border);
        font-family:var(--font-money);font-size:10px;color:var(--text-faint);}
      `;
      function useCSS() {
        React.useEffect(() => {
          if (document.getElementById("nk-cite-css")) return;
          const s = document.createElement("style");
          s.id = "nk-cite-css";
          s.textContent = CSS;
          document.head.appendChild(s);
        }, []);
      }
      function Citation({
        variant = "inline",
        index,
        source,
        fn,
        lines = [],
        total = null,
        className = "",
        ...rest
      }) {
        useCSS();
        if (variant === "tool") {
          return /*#__PURE__*/ React.createElement(
            "div",
            _extends(
              {
                className: ["nk-tool", className].filter(Boolean).join(" "),
              },
              rest,
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "nk-tool__bar",
              },
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "nk-tool__badge",
                },
                "calc",
              ),
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "nk-tool__fn",
                },
                fn,
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "nk-tool__body",
              },
              lines.map((l, i) =>
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "nk-tool__line",
                    key: i,
                  },
                  /*#__PURE__*/ React.createElement("span", null, l.label),
                  /*#__PURE__*/ React.createElement("span", null, l.value),
                ),
              ),
              total
                ? /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "nk-tool__line nk-tool__total",
                    },
                    /*#__PURE__*/ React.createElement("span", null, total.label),
                    /*#__PURE__*/ React.createElement("span", null, total.value),
                  )
                : null,
            ),
            source
              ? /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "nk-tool__src",
                  },
                  /*#__PURE__*/ React.createElement(
                    "svg",
                    {
                      width: "11",
                      height: "11",
                      viewBox: "0 0 24 24",
                      fill: "none",
                      stroke: "currentColor",
                      strokeWidth: "2.2",
                    },
                    /*#__PURE__*/ React.createElement("rect", {
                      x: "3",
                      y: "3",
                      width: "18",
                      height: "18",
                      rx: "2",
                    }),
                    /*#__PURE__*/ React.createElement("path", {
                      d: "M3 9h18M9 3v18",
                    }),
                  ),
                  source,
                )
              : null,
          );
        }
        return /*#__PURE__*/ React.createElement(
          "span",
          _extends(
            {
              className: ["nk-cite", className].filter(Boolean).join(" "),
            },
            rest,
          ),
          index != null
            ? /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "nk-cite__n",
                },
                index,
              )
            : null,
          source,
        );
      }
      Object.assign(__ds_scope, { Citation });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/copilot/Citation.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/copilot/EmptyState.jsx
  try {
    (() => {
      const CSS = `
      .nk-state{display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center;
        gap:12px;padding:36px 28px;font-family:var(--font-sans);max-width:380px;margin:0 auto;}
      .nk-state__ic{width:44px;height:44px;border-radius:var(--radius-md);display:flex;align-items:center;
        justify-content:center;}
      .nk-state__ic--empty{background:var(--surface-elevated);border:1px solid var(--border);color:var(--text-faint);}
      .nk-state__ic--error{background:var(--danger-tint);color:var(--danger-400);}
      .nk-state__title{font-size:15px;font-weight:700;color:var(--text-strong);}
      .nk-state__desc{font-size:13px;line-height:1.5;color:var(--text-muted);}
      .nk-state__action{margin-top:4px;}
      .nk-state__spin{width:30px;height:30px;border-radius:50%;border:2.5px solid var(--border);
        border-top-color:var(--primary);animation:nk-spin .8s linear infinite;}
      @keyframes nk-spin{to{transform:rotate(360deg)}}
      @media (prefers-reduced-motion:reduce){.nk-state__spin{animation:none;}}
      .nk-skel{display:flex;flex-direction:column;gap:9px;width:100%;padding:16px;}
      .nk-skel__row{height:13px;border-radius:5px;background:linear-gradient(90deg,var(--surface-2) 25%,var(--surface-hover) 37%,var(--surface-2) 63%);
        background-size:400% 100%;animation:nk-shimmer 1.4s ease infinite;}
      @keyframes nk-shimmer{0%{background-position:100% 0}100%{background-position:0 0}}
      @media (prefers-reduced-motion:reduce){.nk-skel__row{animation:none;}}
      `;
      function useCSS() {
        React.useEffect(() => {
          if (document.getElementById("nk-state-css")) return;
          const s = document.createElement("style");
          s.id = "nk-state-css";
          s.textContent = CSS;
          document.head.appendChild(s);
        }, []);
      }
      function EmptyState({
        variant = "empty",
        icon = null,
        title,
        description,
        action = null,
        skeletonRows = 4,
        className = "",
      }) {
        useCSS();
        if (variant === "skeleton") {
          return /*#__PURE__*/ React.createElement(
            "div",
            {
              className: ["nk-skel", className].filter(Boolean).join(" "),
            },
            Array.from({
              length: skeletonRows,
            }).map((_, i) =>
              /*#__PURE__*/ React.createElement("div", {
                className: "nk-skel__row",
                key: i,
                style: {
                  width: `${100 - (i % 3) * 14}%`,
                },
              }),
            ),
          );
        }
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: ["nk-state", className].filter(Boolean).join(" "),
          },
          variant === "loading"
            ? /*#__PURE__*/ React.createElement("div", {
                className: "nk-state__spin",
              })
            : /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: `nk-state__ic nk-state__ic--${variant === "error" ? "error" : "empty"}`,
                },
                icon ??
                  /*#__PURE__*/ React.createElement(
                    "svg",
                    {
                      width: "22",
                      height: "22",
                      viewBox: "0 0 24 24",
                      fill: "none",
                      stroke: "currentColor",
                      strokeWidth: "2",
                      strokeLinecap: "round",
                      strokeLinejoin: "round",
                    },
                    variant === "error"
                      ? /*#__PURE__*/ React.createElement(
                          React.Fragment,
                          null,
                          /*#__PURE__*/ React.createElement("circle", {
                            cx: "12",
                            cy: "12",
                            r: "9",
                          }),
                          /*#__PURE__*/ React.createElement("path", {
                            d: "M12 8v4M12 16h.01",
                          }),
                        )
                      : /*#__PURE__*/ React.createElement(
                          React.Fragment,
                          null,
                          /*#__PURE__*/ React.createElement("rect", {
                            x: "3",
                            y: "4",
                            width: "18",
                            height: "16",
                            rx: "2",
                          }),
                          /*#__PURE__*/ React.createElement("path", {
                            d: "M3 10h18M8 4v16",
                          }),
                        ),
                  ),
              ),
          title
            ? /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "nk-state__title",
                },
                title,
              )
            : null,
          description
            ? /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "nk-state__desc",
                },
                description,
              )
            : null,
          action
            ? /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "nk-state__action",
                },
                action,
              )
            : null,
        );
      }
      Object.assign(__ds_scope, { EmptyState });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/copilot/EmptyState.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/copilot/MiaAvatar.jsx
  try {
    (() => {
      // MiaAvatar — Mia's brand mark rendered as an inline SVG.
      // Self-contained; inline-style pattern (no injected stylesheet needed for an SVG).
      // The cat-ear silhouette with jade fill on a dark surface-elevated background.

      function MiaAvatar({ width = 40, height = 40, className = "", style = {} }) {
        return /*#__PURE__*/ React.createElement(
          "svg",
          {
            viewBox: "0 0 40 40",
            fill: "none",
            xmlns: "http://www.w3.org/2000/svg",
            role: "img",
            "aria-label": "Mia, copiloto financeiro",
            width: width,
            height: height,
            className: className,
            style: style,
            focusable: "false",
          },
          /*#__PURE__*/ React.createElement("rect", {
            width: "40",
            height: "40",
            rx: "12",
            fill: "var(--surface-elevated, #1F2827)",
          }),
          /*#__PURE__*/ React.createElement("path", {
            fillRule: "evenodd",
            clipRule: "evenodd",
            fill: "var(--primary, #3FBF8F)",
            d: "M11 15 L9 5 L17.5 11.5 C19 11 21 11 22.5 11.5 L31 5 L29 15 C31.5 17.5 32.5 20.3 32.5 23 C32.5 29 27.5 33.5 20 33.5 C12.5 33.5 7.5 29 7.5 23 C7.5 20.3 8.5 17.5 11 15 Z M16.6 22 C16.6 23.3 15.9 24.3 15 24.3 C14.1 24.3 13.4 23.3 13.4 22 C13.4 20.7 14.1 19.7 15 19.7 C15.9 19.7 16.6 20.7 16.6 22 Z M26.6 22 C26.6 23.3 25.9 24.3 25 24.3 C24.1 24.3 23.4 23.3 23.4 22 C23.4 20.7 24.1 19.7 25 19.7 C25.9 19.7 26.6 20.7 26.6 22 Z M20 26 L18.4 24.7 C18.9 24.3 21.1 24.3 21.6 24.7 Z",
          }),
        );
      }
      Object.assign(__ds_scope, { MiaAvatar });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/copilot/MiaAvatar.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/core/Badge.jsx
  try {
    (() => {
      function _extends() {
        return (
          (_extends = Object.assign
            ? Object.assign.bind()
            : function (n) {
                for (var e = 1; e < arguments.length; e++) {
                  var t = arguments[e];
                  for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]);
                }
                return n;
              }),
          _extends.apply(null, arguments)
        );
      }
      // Inline-style pattern — matches production Badge.tsx which uses no CSS classes.

      const TONE_STYLES = {
        success: {
          background: "var(--success-tint)",
          color: "var(--success-400)",
        },
        warning: {
          background: "var(--warning-tint)",
          color: "var(--warning-400)",
        },
        danger: {
          background: "var(--danger-tint)",
          color: "var(--danger-400)",
        },
        info: {
          background: "var(--info-tint)",
          color: "var(--info-400)",
        },
        primary: {
          background: "var(--primary-quiet)",
          color: "var(--primary)",
        },
        secondary: {
          background: "var(--secondary-quiet)",
          color: "var(--secondary)",
        },
      };
      const BASE = {
        display: "inline-flex",
        alignItems: "center",
        gap: "4px",
        padding: "1px 7px",
        fontSize: "var(--fs-micro)",
        fontWeight: "var(--fw-bold)",
        letterSpacing: "var(--ls-caps)",
        textTransform: "uppercase",
        lineHeight: 1.3,
        whiteSpace: "nowrap",
      };
      const DOT_BASE = {
        width: 6,
        height: 6,
        borderRadius: "50%",
        background: "currentColor",
        display: "inline-block",
        flexShrink: 0,
      };
      function Badge({
        tone = "primary",
        dot = false,
        square = false,
        children,
        className = "",
        ...rest
      }) {
        const toneStyle = TONE_STYLES[tone] ?? TONE_STYLES["primary"];
        const style = {
          ...BASE,
          borderRadius: square ? "4px" : "999px",
          ...toneStyle,
        };
        return /*#__PURE__*/ React.createElement(
          "span",
          _extends(
            {
              className: className,
              style: style,
            },
            rest,
          ),
          dot &&
            /*#__PURE__*/ React.createElement("span", {
              style: DOT_BASE,
            }),
          children,
        );
      }
      Object.assign(__ds_scope, { Badge });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/core/Badge.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/core/Button.jsx
  try {
    (() => {
      function _extends() {
        return (
          (_extends = Object.assign
            ? Object.assign.bind()
            : function (n) {
                for (var e = 1; e < arguments.length; e++) {
                  var t = arguments[e];
                  for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]);
                }
                return n;
              }),
          _extends.apply(null, arguments)
        );
      }
      const CSS = `
      .nk-btn{--_h:var(--hit-min);--_px:14px;--_fs:var(--fs-sm);display:inline-flex;align-items:center;justify-content:center;gap:var(--space-2);
        height:var(--_h);padding:0 var(--_px);font-family:var(--font-sans);font-size:var(--_fs);font-weight:var(--fw-semibold);
        line-height:1;border-radius:var(--radius-sm);border:var(--bw-hair) solid transparent;cursor:pointer;white-space:nowrap;
        transition:var(--t-hover);}
      .nk-btn:active:not([disabled]){transform:translateY(0.5px) scale(0.992);}
      .nk-btn:focus-visible{outline:none;box-shadow:0 0 0 2px var(--bg),0 0 0 4px var(--focus-ring);}
      .nk-btn[disabled]{opacity:.5;cursor:not-allowed;}
      .nk-btn--sm{--_h:28px;--_px:10px;--_fs:var(--fs-sm);}
      .nk-btn--lg{--_h:44px;--_px:18px;--_fs:var(--fs-body);}
      .nk-btn__ic{display:inline-flex;width:16px;height:16px;flex:none;}
      .nk-btn--primary{background:var(--primary);color:var(--text-on-primary);}
      .nk-btn--primary:hover:not([disabled]){background:var(--primary-hover);}
      .nk-btn--primary:active:not([disabled]){background:var(--primary-press);}
      .nk-btn--secondary{background:var(--secondary-quiet);color:var(--secondary);border-color:transparent;}
      .nk-btn--secondary:hover:not([disabled]){filter:brightness(1.06);}
      .nk-btn--ghost{background:transparent;color:var(--text);border-color:var(--border);}
      .nk-btn--ghost:hover:not([disabled]){background:var(--surface-hover);}
      .nk-btn--danger{background:var(--danger-tint);color:var(--danger-400);border-color:transparent;}
      .nk-btn--danger:hover:not([disabled]){filter:brightness(1.08);}
      @media(prefers-reduced-motion:reduce){.nk-btn,.nk-btn:active:not([disabled]){transition:none;transform:none;}}
      `;
      function useCSS() {
        React.useEffect(() => {
          if (document.getElementById("nk-btn-css")) return;
          const s = document.createElement("style");
          s.id = "nk-btn-css";
          s.textContent = CSS;
          document.head.appendChild(s);
        }, []);
      }
      function Button({
        variant = "primary",
        size = "md",
        iconLeft = null,
        iconRight = null,
        disabled = false,
        type = "button",
        className = "",
        children,
        ...rest
      }) {
        useCSS();
        const cls = [
          "nk-btn",
          `nk-btn--${variant}`,
          size !== "md" ? `nk-btn--${size}` : "",
          className,
        ]
          .filter(Boolean)
          .join(" ");
        return /*#__PURE__*/ React.createElement(
          "button",
          _extends(
            {
              type: type,
              className: cls,
              disabled: disabled,
            },
            rest,
          ),
          iconLeft
            ? /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "nk-btn__ic",
                },
                iconLeft,
              )
            : null,
          children ? /*#__PURE__*/ React.createElement("span", null, children) : null,
          iconRight
            ? /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "nk-btn__ic",
                },
                iconRight,
              )
            : null,
        );
      }
      Object.assign(__ds_scope, { Button });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/core/Button.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/core/Disclosure.jsx
  try {
    (() => {
      function _extends() {
        return (
          (_extends = Object.assign
            ? Object.assign.bind()
            : function (n) {
                for (var e = 1; e < arguments.length; e++) {
                  var t = arguments[e];
                  for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]);
                }
                return n;
              }),
          _extends.apply(null, arguments)
        );
      }
      const CSS = `
      .nk-disc{font-family:var(--font-sans);width:100%;}

      /* card variant (bare=false): own background, border, shadow */
      .nk-disc:not(.nk-disc--bare){
        background:var(--surface);
        border:var(--bw-default) solid var(--border);
        border-radius:var(--radius-md);
        box-shadow:var(--shadow-1);
        overflow:hidden;
      }

      /* accent left border strip */
      .nk-disc--ok:not(.nk-disc--bare){border-left:3px solid var(--success-500);}
      .nk-disc--warn:not(.nk-disc--bare){border-left:3px solid var(--warning-500);}
      .nk-disc--brass:not(.nk-disc--bare){border-left:3px solid var(--secondary);}

      /* trigger button */
      .nk-disc__head{
        display:flex;
        align-items:center;
        gap:10px;
        width:100%;
        padding:12px 14px;
        background:transparent;
        border:none;
        cursor:pointer;
        text-align:left;
        color:var(--text);
        font-family:inherit;
        font-size:var(--fs-body);
        line-height:var(--lh-snug);
        border-radius:inherit;
        transition:background-color var(--dur-fast) var(--ease-standard);
        min-height:var(--hit-min);
      }
      .nk-disc__head:hover{background:var(--surface-hover);}
      .nk-disc__head:focus-visible{
        outline:none;
        box-shadow:var(--shadow-focus);
        border-radius:var(--radius-sm);
      }

      /* icon slot */
      .nk-disc__ic{
        display:inline-flex;
        align-items:center;
        justify-content:center;
        flex:none;
        width:20px;
        height:20px;
        color:var(--text-faint);
      }

      /* text column */
      .nk-disc__titles{
        display:flex;
        flex-direction:column;
        gap:2px;
        flex:1;
        min-width:0;
      }

      .nk-disc__title{
        display:flex;
        align-items:center;
        gap:8px;
        font-size:var(--fs-body);
        font-weight:var(--fw-semibold);
        color:var(--text-strong);
        white-space:nowrap;
        overflow:hidden;
        text-overflow:ellipsis;
      }

      .nk-disc__summary{
        font-size:var(--fs-sm);
        color:var(--text-muted);
        white-space:nowrap;
        overflow:hidden;
        text-overflow:ellipsis;
      }

      /* chevron */
      .nk-disc__chev{
        flex:none;
        color:var(--text-faint);
        transition:transform var(--dur-base) var(--ease-standard);
        margin-left:auto;
      }
      @media (prefers-reduced-motion:reduce){
        .nk-disc__chev{transition:none;}
      }
      .nk-disc.is-open .nk-disc__chev{transform:rotate(180deg);}

      /* body wrapper: grid-rows collapse trick — no height-jank */
      .nk-disc__bodywrap{
        display:grid;
        grid-template-rows:0fr;
        transition:grid-template-rows var(--dur-base) var(--ease-standard);
        overflow:hidden;
      }
      @media (prefers-reduced-motion:reduce){
        .nk-disc__bodywrap{transition:none;}
      }
      .nk-disc.is-open .nk-disc__bodywrap{grid-template-rows:1fr;}

      /* inner must have min-height:0 for grid trick */
      .nk-disc__body{
        min-height:0;
        overflow:hidden;
      }

      /* bare variant: inner content gets standard padding */
      .nk-disc--bare .nk-disc__body > *{
        padding-top:0;
      }

      /* card variant: divider + padding inside body */
      .nk-disc:not(.nk-disc--bare) .nk-disc__body{
        border-top:var(--bw-hair) solid var(--border);
        padding:14px;
      }

      /* accent ok/warn/brass on bare — subtle tinted title text */
      .nk-disc--bare.nk-disc--ok .nk-disc__title{color:var(--success-400);}
      .nk-disc--bare.nk-disc--warn .nk-disc__title{color:var(--warning-400);}
      .nk-disc--bare.nk-disc--brass .nk-disc__title{color:var(--secondary);}
      `;
      function useCSS() {
        React.useEffect(() => {
          if (document.getElementById("nk-disc-css")) return;
          const s = document.createElement("style");
          s.id = "nk-disc-css";
          s.textContent = CSS;
          document.head.appendChild(s);
        }, []);
      }
      let _idCounter = 0;
      function useStableId() {
        const [id] = React.useState(() => `nk-disc-${++_idCounter}`);
        return id;
      }
      function Chevron() {
        return /*#__PURE__*/ React.createElement(
          "svg",
          {
            className: "nk-disc__chev",
            width: 16,
            height: 16,
            viewBox: "0 0 24 24",
            fill: "none",
            "aria-hidden": "true",
          },
          /*#__PURE__*/ React.createElement("path", {
            d: "M6 9l6 6 6-6",
            stroke: "currentColor",
            strokeWidth: 1.75,
            strokeLinecap: "round",
            strokeLinejoin: "round",
          }),
        );
      }
      function Disclosure({
        title = "Detalhes da transação",
        summary,
        icon,
        accent,
        badge,
        defaultOpen = false,
        bare = true,
        children,
        className = "",
      }) {
        useCSS();
        const [open, setOpen] = React.useState(defaultOpen);
        const id = useStableId();
        const classes = [
          "nk-disc",
          bare ? "nk-disc--bare" : "",
          open ? "is-open" : "",
          accent ? `nk-disc--${accent}` : "",
          className,
        ]
          .filter(Boolean)
          .join(" ");
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: classes,
          },
          /*#__PURE__*/ React.createElement(
            "button",
            {
              type: "button",
              className: "nk-disc__head",
              "aria-expanded": open,
              "aria-controls": `${id}-b`,
              onClick: () => setOpen((o) => !o),
            },
            icon
              ? /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "nk-disc__ic",
                  },
                  icon,
                )
              : null,
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "nk-disc__titles",
              },
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "nk-disc__title",
                  id: `${id}-t`,
                },
                title,
                badge,
              ),
              summary
                ? /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      className: "nk-disc__summary",
                    },
                    summary,
                  )
                : null,
            ),
            /*#__PURE__*/ React.createElement(Chevron, null),
          ),
          /*#__PURE__*/ React.createElement(
            "section",
            _extends(
              {
                className: "nk-disc__bodywrap",
                id: `${id}-b`,
                "aria-labelledby": `${id}-t`,
                role: "region",
              },
              !open
                ? {
                    inert: "",
                  }
                : {},
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "nk-disc__body",
              },
              children ??
                /*#__PURE__*/ React.createElement(
                  "p",
                  {
                    style: {
                      margin: 0,
                      color: "var(--text-muted)",
                      fontSize: "var(--fs-sm)",
                    },
                  },
                  "Nenhum detalhe dispon\xEDvel.",
                ),
            ),
          ),
        );
      }
      Object.assign(__ds_scope, { Disclosure });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/core/Disclosure.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/core/InfoPopover.jsx
  try {
    (() => {
      // InfoPopover — didactic term explainer for the Neko method (PT-BR glossary).
      // Wraps a trigger label; opens a positioned popover on click/hover/keyboard.
      // Portal-free in the DS recreation: uses fixed positioning relative to the
      // wrapper's bounding rect (same visual result without createPortal).
      // CSS-injection pattern (like MetricTile/ChatBubble).

      const CSS = `
      .nk-pop-wrap{position:relative;display:inline-flex;}

      /* Trigger button */
      .nk-term{
        display:inline-flex;align-items:center;gap:4px;
        background:none;border:none;padding:0;margin:0;cursor:pointer;
        font-family:inherit;font-size:inherit;font-weight:inherit;color:inherit;
        text-decoration:underline;text-decoration-color:var(--border-strong);
        text-underline-offset:2px;text-decoration-style:dotted;
        border-radius:var(--radius-xs);
        transition:var(--t-hover);
      }
      .nk-term:hover,.nk-term:focus-visible{
        color:var(--primary);
        text-decoration-color:var(--primary);
        outline:none;
      }
      .nk-term:focus-visible{
        box-shadow:var(--shadow-focus);
      }
      .nk-term--plain{
        text-decoration:none;
      }

      /* "i" marker badge */
      .nk-term__i{
        display:inline-flex;align-items:center;justify-content:center;
        width:13px;height:13px;flex:none;
        border-radius:var(--radius-circle);
        background:var(--surface-2);
        border:1px solid var(--border-strong);
        font-family:var(--font-sans);
        font-size:9px;font-weight:700;
        color:var(--text-faint);
        letter-spacing:0;
        line-height:1;
        vertical-align:middle;
        user-select:none;
      }
      .nk-term:hover .nk-term__i,
      .nk-term:focus-visible .nk-term__i{
        background:var(--primary-quiet);
        border-color:var(--primary);
        color:var(--primary);
      }

      /* Popover panel */
      .nk-pop{
        position:fixed;z-index:9000;
        display:flex;flex-direction:column;gap:5px;
        padding:12px 14px;
        background:var(--surface-elevated);
        border:1px solid var(--border-strong);
        border-radius:var(--radius-md);
        box-shadow:var(--elev-overlay);
        pointer-events:auto;
      }

      /* Fade-in animation */
      .nk-pop{
        animation:nk-pop-in var(--dur-fast) var(--ease-entrance) both;
      }
      @keyframes nk-pop-in{
        from{opacity:0;transform:translateY(3px);}
        to{opacity:1;transform:translateY(0);}
      }
      .nk-pop--top{
        animation:nk-pop-in-top var(--dur-fast) var(--ease-entrance) both;
      }
      @keyframes nk-pop-in-top{
        from{opacity:0;transform:translateY(-3px);}
        to{opacity:1;transform:translateY(0);}
      }
      @media(prefers-reduced-motion:reduce){
        .nk-pop,.nk-pop--top{animation:none;}
      }

      /* Arrow */
      .nk-pop::before{
        content:"";position:absolute;
        width:10px;height:10px;
        background:var(--surface-elevated);
        border:1px solid var(--border-strong);
        transform:rotate(45deg);
        left:var(--arrow-x,12px);
      }
      .nk-pop--bottom::before{
        top:-6px;
        border-bottom-color:transparent;border-right-color:transparent;
      }
      .nk-pop--top::before{
        bottom:-6px;
        border-top-color:transparent;border-left-color:transparent;
      }

      /* Popover content */
      .nk-pop__title{
        font-family:var(--font-sans);
        font-size:12px;font-weight:700;
        color:var(--text-strong);
        letter-spacing:var(--ls-snug);
        line-height:1.2;
      }
      .nk-pop__body{
        font-family:var(--font-sans);
        font-size:13px;
        color:var(--text);
        line-height:var(--lh-relaxed);
      }
      .nk-pop__hint{
        font-family:var(--font-sans);
        font-size:11px;
        color:var(--text-faint);
        margin-top:2px;
      }
      `;

      /** Canonical PT-BR glossary for the Neko method. */
      const GLOSSARY = {
        pode_gastar: {
          title: "Pode gastar hoje",
          body: "O quanto dá para gastar hoje sem furar o mês. É o menor de dois limites: o que o caixa aguenta e o que respeita sua meta de poupança.",
        },
        piso_caixa: {
          title: "Limite do caixa",
          body: "O máximo por dia que mantém nenhum dia do mês no vermelho, olhando o saldo projetado.",
        },
        folga_poupanca: {
          title: "Limite da poupança",
          body: "O máximo por dia que ainda deixa você guardar a meta do ano (20% a 30% da renda).",
        },
        reserva: {
          title: "Reserva",
          body: "Quantos meses de custo de vida você cobre com o que tem guardado. A meta mínima é 6 meses; a partir de 12 é a 'paz' financeira.",
        },
        caixa: {
          title: "Caixa",
          body: "É dinheiro de passagem, não a sua riqueza. O que está na conta hoje, antes das contas do mês.",
        },
        previsibilidade: {
          title: "Previsibilidade",
          body: "O quanto do gasto típico de cada mês futuro já está lançado. Futuro vazio engana a previsão.",
        },
        colchao: {
          title: "Colchão",
          body: "O que sobra e você guarda para cobrir meses negativos sem sacar investimento. Adaptação válida do método.",
        },
        performance: {
          title: "Performance",
          body: "A foto do mês: Entradas menos Saídas (incluem fixas e fatura do cartão) e Diário. É o mesmo cálculo da sua planilha.",
        },
        economizado: {
          title: "Economizado",
          body: "Quanto da renda você guardou como Economia. A meta do método é de 20% a 30% no ano.",
        },
        custo_de_vida: {
          title: "Custo de vida",
          body: "Saídas fixas, diário e cartão somados. O que custa manter sua vida no mês.",
        },
        diario_medio: {
          title: "Diário médio",
          body: "A média do gasto variável por dia até hoje. Ajuda a saber se o ritmo do mês está saudável.",
        },
        cartao: {
          title: "Cartão",
          body: "Compras no cartão viram fatura no vencimento. Gastar hoje no crédito afunda os meses à frente.",
        },
      };
      function useCSS() {
        React.useEffect(() => {
          if (document.getElementById("nk-pop-css")) return;
          const s = document.createElement("style");
          s.id = "nk-pop-css";
          s.textContent = CSS;
          document.head.appendChild(s);
        }, []);
      }

      // Simple counter for unique IDs without useId (DS bundle compatibility).
      let _idCounter = 0;
      function useSafeId() {
        const ref = React.useRef(null);
        if (ref.current === null) ref.current = `nk-pop-${++_idCounter}`;
        return ref.current;
      }
      function InfoPopover({
        term = "reserva",
        children,
        hideMarker = false,
        width = 280,
        className = "",
      }) {
        useCSS();
        const entry =
          typeof term === "string"
            ? (GLOSSARY[term] ?? {
                body: term,
              })
            : term;
        const [open, setOpen] = React.useState(false);
        const [pos, setPos] = React.useState(null);
        const wrapRef = React.useRef(null);
        const popRef = React.useRef(null);
        const hoverTimer = React.useRef(undefined);
        const id = useSafeId();
        React.useEffect(() => {
          if (!open) return;
          const dismiss = () => setOpen(false);
          const dismissAndRefocus = () => {
            dismiss();
            wrapRef.current?.querySelector(".nk-term")?.focus();
          };
          const place = () => {
            const trigger = wrapRef.current?.querySelector(".nk-term");
            if (!trigger) return;
            const r = trigger.getBoundingClientRect();
            const MARGIN = 12;
            const GAP = 9;
            const popH = popRef.current ? popRef.current.offsetHeight : 96;
            const below =
              r.bottom + GAP + popH + MARGIN <= window.innerHeight ||
              r.top - GAP - popH < MARGIN;
            const top = below ? r.bottom + GAP : r.top - GAP - popH;
            let left = r.left;
            left = Math.min(left, window.innerWidth - width - MARGIN);
            left = Math.max(MARGIN, left);
            const arrowX = Math.max(
              12,
              Math.min(width - 20, r.left + r.width / 2 - left),
            );
            setPos({
              left,
              top,
              side: below ? "bottom" : "top",
              arrowX,
            });
          };
          place();
          const raf = requestAnimationFrame(place);
          const onKey = (e) => {
            if (e.key === "Escape") dismissAndRefocus();
          };
          const onDoc = (e) => {
            const t = e.target;
            if (wrapRef.current?.contains(t) || popRef.current?.contains(t)) return;
            dismiss();
          };
          const onScroll = () => place();
          document.addEventListener("keydown", onKey);
          document.addEventListener("mousedown", onDoc);
          window.addEventListener("scroll", onScroll, true);
          window.addEventListener("resize", onScroll);
          return () => {
            cancelAnimationFrame(raf);
            document.removeEventListener("keydown", onKey);
            document.removeEventListener("mousedown", onDoc);
            window.removeEventListener("scroll", onScroll, true);
            window.removeEventListener("resize", onScroll);
          };
        }, [open, width]);

        // Clean up hover timer on unmount.
        React.useEffect(() => () => clearTimeout(hoverTimer.current), []);
        const show = () => {
          clearTimeout(hoverTimer.current);
          setOpen(true);
        };
        const hideSoon = () => {
          clearTimeout(hoverTimer.current);
          hoverTimer.current = setTimeout(() => setOpen(false), 140);
        };
        const marker = !hideMarker
          ? /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "nk-term__i",
                "aria-hidden": "true",
              },
              "i",
            )
          : null;

        // Default demo children when none supplied.
        const trigger =
          children ?? (typeof term === "string" ? term.replace(/_/g, " ") : "termo");
        return /*#__PURE__*/ React.createElement(
          "span",
          {
            className: "nk-pop-wrap",
            ref: wrapRef,
            onMouseEnter: show,
            onMouseLeave: hideSoon,
          },
          /*#__PURE__*/ React.createElement(
            "button",
            {
              type: "button",
              className: ["nk-term", hideMarker ? "nk-term--plain" : "", className]
                .filter(Boolean)
                .join(" "),
              "aria-describedby": open ? id : undefined,
              onClick: (e) => {
                e.stopPropagation();
                setOpen(true);
              },
            },
            trigger,
            marker,
          ),
          open && pos
            ? (() => {
                // DS preview renders into the same page — no createPortal available.
                // We render inside the wrapper but use `position:fixed` so the
                // popover escapes overflow clipping.
                return /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: `nk-pop nk-pop--${pos.side}`,
                    role: "tooltip",
                    id: id,
                    ref: popRef,
                    style: {
                      left: `${pos.left}px`,
                      top: `${pos.top}px`,
                      width: `${width}px`,
                      "--arrow-x": `${pos.arrowX}px`,
                    },
                    onMouseEnter: show,
                    onMouseLeave: hideSoon,
                  },
                  entry.title
                    ? /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          className: "nk-pop__title",
                        },
                        entry.title,
                      )
                    : null,
                  /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      className: "nk-pop__body",
                    },
                    entry.body,
                  ),
                  /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      className: "nk-pop__hint",
                    },
                    "Esc para fechar",
                  ),
                );
              })()
            : null,
        );
      }
      Object.assign(__ds_scope, { InfoPopover });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/core/InfoPopover.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/core/Input.jsx
  try {
    (() => {
      function _extends() {
        return (
          (_extends = Object.assign
            ? Object.assign.bind()
            : function (n) {
                for (var e = 1; e < arguments.length; e++) {
                  var t = arguments[e];
                  for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]);
                }
                return n;
              }),
          _extends.apply(null, arguments)
        );
      }
      // Tokens used:
      //   --font-sans, --font-money
      //   --fs-label, --fs-body, --fs-sm, --fs-micro
      //   --fw-semibold
      //   --ls-label
      //   --text, --text-muted, --text-faint
      //   --bg-subtle, --surface-2
      //   --border-input, --border-strong, --border-focus
      //   --danger-400, --danger-500, --danger-tint
      //   --focus-ring
      //   --radius-xs
      //   --hit-min, --bw-hair
      //   --space-3, --space-2
      //   --t-hover, --dur-fast, --ease-standard

      const CSS = `
      .nk-field{display:flex;flex-direction:column;gap:var(--space-2);font-family:var(--font-sans);}
      .nk-field__label{font-size:var(--fs-label);font-weight:var(--fw-semibold);color:var(--text-muted);
        letter-spacing:var(--ls-label);text-transform:uppercase;}
      .nk-field__req{color:var(--danger-400);margin-left:3px;}
      .nk-input{display:flex;align-items:center;gap:var(--space-2);height:var(--hit-min);
        padding:0 var(--space-3);background:var(--bg-subtle);
        border:var(--bw-hair) solid var(--border-input);border-radius:var(--radius-xs);
        transition:var(--t-hover),box-shadow var(--dur-fast) var(--ease-standard);}
      .nk-input:hover{border-color:var(--border-strong);}
      .nk-input:focus-within{border-color:var(--border-focus);box-shadow:0 0 0 3px var(--focus-ring);}
      .nk-input--err{border-color:var(--danger-500);}
      .nk-input--err:focus-within{box-shadow:0 0 0 3px var(--danger-tint);}
      .nk-input input{flex:1;min-width:0;background:none;border:none;outline:none;color:var(--text);
        font-family:inherit;font-size:var(--fs-body);}
      .nk-input input::placeholder{color:var(--text-faint);}
      .nk-input--money input{font-family:var(--font-money);font-variant-numeric:tabular-nums;text-align:right;}
      .nk-input__affix{color:var(--text-faint);font-size:var(--fs-sm);display:inline-flex;align-items:center;flex:none;}
      .nk-input__icon{width:16px;height:16px;color:var(--text-faint);flex:none;display:inline-flex;}
      .nk-input--disabled,.nk-input[disabled]{opacity:.5;pointer-events:none;}
      .nk-field__hint{font-size:var(--fs-micro);color:var(--text-faint);}
      .nk-field__hint--err{color:var(--danger-400);}
      .nk-input--readonly{background:var(--surface-2);color:var(--text-muted);}
      `;
      function useCSS() {
        React.useEffect(() => {
          if (document.getElementById("nk-input-css")) return;
          const s = document.createElement("style");
          s.id = "nk-input-css";
          s.textContent = CSS;
          document.head.appendChild(s);
        }, []);
      }
      function Input({
        label,
        required = false,
        prefix = null,
        suffix = null,
        icon = null,
        money = false,
        readOnly = false,
        error = "",
        hint = "",
        disabled = false,
        className = "",
        id,
        ...rest
      }) {
        useCSS();
        // useId must be called unconditionally (Rules of Hooks); fall back to the external id after.
        const autoId = React.useId();
        const fid = id || autoId;
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: ["nk-field", className].filter(Boolean).join(" "),
          },
          label
            ? /*#__PURE__*/ React.createElement(
                "label",
                {
                  className: "nk-field__label",
                  htmlFor: fid,
                },
                label,
                required
                  ? /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        className: "nk-field__req",
                      },
                      "*",
                    )
                  : null,
              )
            : null,
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: [
                "nk-input",
                money ? "nk-input--money" : "",
                error ? "nk-input--err" : "",
                disabled ? "nk-input--disabled" : "",
                readOnly ? "nk-input--readonly" : "",
              ]
                .filter(Boolean)
                .join(" "),
            },
            icon
              ? /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "nk-input__icon",
                  },
                  icon,
                )
              : null,
            prefix
              ? /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "nk-input__affix",
                  },
                  prefix,
                )
              : null,
            /*#__PURE__*/ React.createElement(
              "input",
              _extends(
                {
                  id: fid,
                  disabled: disabled,
                  readOnly: readOnly,
                },
                rest,
              ),
            ),
            suffix
              ? /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "nk-input__affix",
                  },
                  suffix,
                )
              : null,
          ),
          error
            ? /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "nk-field__hint nk-field__hint--err",
                },
                error,
              )
            : hint
              ? /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "nk-field__hint",
                  },
                  hint,
                )
              : null,
        );
      }
      Object.assign(__ds_scope, { Input });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/core/Input.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/core/MonthNav.jsx
  try {
    (() => {
      // MonthNav — temporal navigation control "< Mês/Ano >" + "Hoje" shortcut.
      // Self-contained; inline-style pattern (no CSS injection needed).

      // Inline SVG chevrons (Lucide-style, 18×18, strokeWidth 2, round caps/joins).
      function ChevronLeft() {
        return /*#__PURE__*/ React.createElement(
          "svg",
          {
            width: "18",
            height: "18",
            viewBox: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            strokeWidth: "2",
            strokeLinecap: "round",
            strokeLinejoin: "round",
            "aria-hidden": "true",
          },
          /*#__PURE__*/ React.createElement("path", {
            d: "m15 18-6-6 6-6",
          }),
        );
      }
      function ChevronRight() {
        return /*#__PURE__*/ React.createElement(
          "svg",
          {
            width: "18",
            height: "18",
            viewBox: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            strokeWidth: "2",
            strokeLinecap: "round",
            strokeLinejoin: "round",
            "aria-hidden": "true",
          },
          /*#__PURE__*/ React.createElement("path", {
            d: "m9 18 6-6-6-6",
          }),
        );
      }
      function arrowBtnStyle(enabled) {
        return {
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          width: "var(--hit-min)",
          height: "var(--hit-min)",
          borderRadius: "var(--radius-sm)",
          border: "var(--bw-hair) solid var(--border)",
          background: "var(--surface)",
          color: enabled ? "var(--text)" : "var(--text-faint)",
          cursor: enabled ? "pointer" : "not-allowed",
          opacity: enabled ? 1 : 0.5,
          transition: "background-color var(--dur-fast) var(--ease-standard)",
          flexShrink: 0,
        };
      }
      const TODAY_BTN_STYLE = {
        marginLeft: "var(--space-2)",
        padding: "var(--space-2) var(--space-4)",
        borderRadius: "var(--radius-pill)",
        border: "var(--bw-hair) solid var(--border)",
        background: "var(--primary-quiet)",
        color: "var(--primary-quiet-text)",
        fontSize: "var(--fs-sm)",
        fontWeight: "var(--fw-semibold)",
        cursor: "pointer",
        lineHeight: 1.4,
      };
      const LABEL_STYLE = {
        minWidth: 150,
        textAlign: "center",
        fontSize: "var(--fs-title)",
        fontWeight: "var(--fw-bold)",
        letterSpacing: "var(--ls-tight)",
        color: "var(--text-strong)",
        fontFamily: "var(--font-sans)",
      };
      function MonthNav({
        label = "Junho de 2026",
        onPrev = () => {},
        onNext = () => {},
        onToday = () => {},
        canPrev = true,
        canNext = true,
        atToday = false,
        prevLabel = "Mês anterior",
        nextLabel = "Próximo mês",
        className = "",
      }) {
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: className || undefined,
            style: {
              display: "inline-flex",
              alignItems: "center",
              gap: "var(--space-3)",
            },
          },
          /*#__PURE__*/ React.createElement(
            "button",
            {
              type: "button",
              "aria-label": prevLabel,
              disabled: !canPrev,
              onClick: onPrev,
              style: arrowBtnStyle(canPrev),
            },
            /*#__PURE__*/ React.createElement(ChevronLeft, null),
          ),
          /*#__PURE__*/ React.createElement(
            "span",
            {
              "aria-live": "polite",
              style: LABEL_STYLE,
            },
            label,
          ),
          /*#__PURE__*/ React.createElement(
            "button",
            {
              type: "button",
              "aria-label": nextLabel,
              disabled: !canNext,
              onClick: onNext,
              style: arrowBtnStyle(canNext),
            },
            /*#__PURE__*/ React.createElement(ChevronRight, null),
          ),
          !atToday &&
            /*#__PURE__*/ React.createElement(
              "button",
              {
                type: "button",
                onClick: onToday,
                style: TODAY_BTN_STYLE,
              },
              "Hoje",
            ),
        );
      }
      Object.assign(__ds_scope, { MonthNav });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/core/MonthNav.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/core/NekoMark.jsx
  try {
    (() => {
      // NekoMark — app logo mark (cat-face SVG, 48×48 viewBox).
      // Renders with `currentColor`, so tinting via `color` or `className` works.
      // No dependencies, no external imports, no CSS injection needed — pure SVG.

      function NekoMark({
        width = 48,
        height = 48,
        color = "var(--primary)",
        className = "",
        style = {},
        "aria-label": ariaLabel = "Neko",
        "aria-hidden": ariaHidden,
      }) {
        const hasLabel = ariaLabel && ariaHidden !== true && ariaHidden !== "true";
        return /*#__PURE__*/ React.createElement(
          "svg",
          {
            viewBox: "0 0 48 48",
            fill: "none",
            xmlns: "http://www.w3.org/2000/svg",
            role: hasLabel ? "img" : undefined,
            "aria-label": hasLabel ? ariaLabel : undefined,
            "aria-hidden": !hasLabel ? true : undefined,
            width: width,
            height: height,
            className: className,
            style: {
              color,
              flexShrink: 0,
              ...style,
            },
          },
          /*#__PURE__*/ React.createElement("path", {
            fill: "currentColor",
            fillRule: "evenodd",
            clipRule: "evenodd",
            d: "M12 17 L9.2 5.4 L20 13.2 C22 12.6 26 12.6 28 13.2 L38.8 5.4 L36 17 C39.4 20 40.5 23.5 40.5 27 C40.5 35 33.5 41.5 24 41.5 C14.5 41.5 7.5 35 7.5 27 C7.5 23.5 8.6 20 12 17 Z M18.5 25.5 C18.5 27.2 17.6 28.5 16.4 28.5 C15.2 28.5 14.3 27.2 14.3 25.5 C14.3 23.8 15.2 22.5 16.4 22.5 C17.6 22.5 18.5 23.8 18.5 25.5 Z M33.7 25.5 C33.7 27.2 32.8 28.5 31.6 28.5 C30.4 28.5 29.5 27.2 29.5 25.5 C29.5 23.8 30.4 22.5 31.6 22.5 C32.8 22.5 33.7 23.8 33.7 25.5 Z M24 30.2 L22 28.6 C22.6 28.1 25.4 28.1 26 28.6 Z",
          }),
        );
      }
      Object.assign(__ds_scope, { NekoMark });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/core/NekoMark.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/core/SegmentedControl.jsx
  try {
    (() => {
      const CSS = `
      .nk-seg{display:inline-flex;padding:2px;background:var(--bg-subtle);
        border-radius:var(--radius-xs);gap:2px;font-family:var(--font-sans);}
      .nk-seg__opt{appearance:none;border:none;background:transparent;cursor:pointer;
        min-height:32px;padding:4px 14px;
        border-radius:calc(var(--radius-xs) - 1px);
        font-family:var(--font-sans);font-size:var(--fs-body);font-weight:var(--fw-medium);
        color:var(--text-muted);white-space:nowrap;transition:var(--t-hover);}
      .nk-seg__opt:hover:not(:disabled){color:var(--text);}
      .nk-seg__opt[aria-checked="true"]{background:var(--surface-selected);color:var(--primary);}
      .nk-seg__opt:focus-visible{outline:none;box-shadow:var(--shadow-focus);}
      .nk-seg__opt:disabled{cursor:not-allowed;opacity:0.5;}
      .nk-seg--sm .nk-seg__opt{min-height:28px;padding:2px 10px;font-size:var(--fs-sm);}
      `;
      function useCSS() {
        React.useEffect(() => {
          if (document.getElementById("nk-seg-css")) return;
          const s = document.createElement("style");
          s.id = "nk-seg-css";
          s.textContent = CSS;
          document.head.appendChild(s);
        }, []);
      }
      function SegmentedControl({
        options = [
          {
            value: "dia",
            label: "Dia",
          },
          {
            value: "semana",
            label: "Semana",
          },
          {
            value: "mes",
            label: "Mês",
          },
        ],
        value = "mes",
        onChange = () => {},
        size = "md",
        className = "",
        disabled = false,
        ariaLabel,
      }) {
        useCSS();
        const handleKeyDown = (e, idx) => {
          if (disabled || options.length === 0) return;
          let next;
          switch (e.key) {
            case "ArrowRight":
            case "ArrowDown":
              next = (idx + 1) % options.length;
              break;
            case "ArrowLeft":
            case "ArrowUp":
              next = (idx - 1 + options.length) % options.length;
              break;
            case "Home":
              next = 0;
              break;
            case "End":
              next = options.length - 1;
              break;
            default:
              return;
          }
          e.preventDefault();
          const target = options[next];
          if (!target) return;
          onChange(target.value);
          const group = e.currentTarget.parentElement;
          const radios = group && group.querySelectorAll('[role="radio"]');
          if (radios && radios[next]) radios[next].focus();
        };
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            role: "radiogroup",
            "aria-label": ariaLabel,
            className: ["nk-seg", size === "sm" ? "nk-seg--sm" : "", className]
              .filter(Boolean)
              .join(" "),
          },
          options.map((opt, idx) => {
            const isActive = value === opt.value;
            return /*#__PURE__*/ React.createElement(
              "button",
              {
                key: opt.value,
                role: "radio",
                type: "button",
                "aria-checked": isActive,
                tabIndex: isActive ? 0 : -1,
                disabled: disabled,
                className: "nk-seg__opt",
                onClick: () => !disabled && onChange(opt.value),
                onKeyDown: (e) => handleKeyDown(e, idx),
              },
              opt.label,
            );
          }),
        );
      }
      Object.assign(__ds_scope, { SegmentedControl });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/core/SegmentedControl.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/core/Switch.jsx
  try {
    (() => {
      function _extends() {
        return (
          (_extends = Object.assign
            ? Object.assign.bind()
            : function (n) {
                for (var e = 1; e < arguments.length; e++) {
                  var t = arguments[e];
                  for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]);
                }
                return n;
              }),
          _extends.apply(null, arguments)
        );
      }
      const CSS = `
      .nk-switch{display:inline-flex;align-items:center;gap:10px;cursor:pointer;font-family:var(--font-sans);
        font-size:var(--fs-body);color:var(--text);user-select:none;}
      .nk-switch input{position:absolute;opacity:0;width:0;height:0;}
      .nk-switch__track{position:relative;width:36px;height:20px;border-radius:10px;
        background:var(--border-input);transition:var(--t-hover);flex:none;}
      [data-theme="light"] .nk-switch__track{background:#727c77;}
      .nk-switch__thumb{position:absolute;top:2px;left:2px;width:16px;height:16px;border-radius:50%;
        background:#fff;transition:var(--t-hover);box-shadow:var(--shadow-1);}
      .nk-switch input:checked + .nk-switch__track{background:var(--primary);}
      .nk-switch input:checked + .nk-switch__track .nk-switch__thumb{left:18px;background:#fff;}
      .nk-switch input:focus-visible + .nk-switch__track{box-shadow:var(--shadow-focus);}
      .nk-switch--disabled{opacity:.45;pointer-events:none;}
      `;
      function useCSS() {
        React.useEffect(() => {
          if (document.getElementById("nk-switch-css")) return;
          const s = document.createElement("style");
          s.id = "nk-switch-css";
          s.textContent = CSS;
          document.head.appendChild(s);
        }, []);
      }
      function Switch({
        checked = false,
        onChange = () => {},
        label,
        disabled = false,
        className = "",
        ...rest
      }) {
        useCSS();
        return /*#__PURE__*/ React.createElement(
          "label",
          {
            className: ["nk-switch", disabled ? "nk-switch--disabled" : "", className]
              .filter(Boolean)
              .join(" "),
          },
          /*#__PURE__*/ React.createElement(
            "input",
            _extends(
              {
                type: "checkbox",
                checked: checked,
                disabled: disabled,
                onChange: (e) => onChange(e.target.checked),
              },
              rest,
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: "nk-switch__track",
            },
            /*#__PURE__*/ React.createElement("span", {
              className: "nk-switch__thumb",
            }),
          ),
          label ? /*#__PURE__*/ React.createElement("span", null, label) : null,
        );
      }
      Object.assign(__ds_scope, { Switch });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/core/Switch.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/finance/ApprovalDiffCard.jsx
  try {
    (() => {
      const CSS = `
      .nk-diff{background:var(--surface);border:var(--bw-hair) solid var(--border);border-radius:var(--radius-md);
        overflow:hidden;font-family:var(--font-sans);box-shadow:var(--shadow-2);max-width:480px;}
      .nk-diff__head{display:flex;align-items:flex-start;gap:11px;padding:14px 16px;border-bottom:var(--bw-hair) solid var(--border);}
      .nk-diff__mark{width:28px;height:28px;border-radius:var(--radius-sm);background:var(--primary-quiet);color:var(--primary);
        display:flex;align-items:center;justify-content:center;flex:none;}
      .nk-diff__htxt{flex:1;min-width:0;}
      .nk-diff__title{display:block;font-size:14px;font-weight:var(--fw-bold);color:var(--text-strong);letter-spacing:-0.005em;}
      .nk-diff__src{font-family:var(--font-money);font-size:var(--fs-label);color:var(--text-faint);margin-top:3px;
        display:flex;align-items:center;gap:6px;flex-wrap:wrap;}
      .nk-diff__src b{color:var(--text-muted);font-weight:var(--fw-semibold);}
      .nk-diff__pill{font-size:var(--fs-label);font-weight:var(--fw-bold);letter-spacing:.05em;text-transform:uppercase;padding:3px 8px;
        border-radius:var(--radius-pill);flex:none;}
      .nk-diff__pill--pending{background:var(--warning-tint);color:var(--warning-400);}
      .nk-diff__pill--approved{background:var(--success-tint);color:var(--success-400);}
      .nk-diff__pill--rejected{background:var(--danger-tint);color:var(--danger-400);}
      .nk-diff__rows{padding:6px 16px 12px;}
      .nk-diff__row{display:grid;grid-template-columns:104px 1fr;gap:10px;padding:8px 0;border-bottom:1px dashed var(--border);align-items:center;}
      .nk-diff__row:last-child{border-bottom:none;}
      .nk-diff__field{font-size:12px;color:var(--text-muted);font-weight:var(--fw-semibold);}
      .nk-diff__vals{display:flex;align-items:center;gap:8px;flex-wrap:wrap;font-family:var(--font-money);
        font-variant-numeric:tabular-nums;font-size:13px;}
      .nk-diff__before{color:var(--diff-remove);background:var(--diff-remove-bg);padding:2px 7px;border-radius:var(--radius-xs);
        text-decoration:line-through;}
      .nk-diff__arrow{color:var(--text-faint);}
      .nk-diff__after{color:var(--diff-add);background:var(--diff-add-bg);padding:2px 7px;border-radius:var(--radius-xs);font-weight:var(--fw-semibold);}
      .nk-diff__note{display:flex;gap:8px;padding:11px 16px;background:var(--bg-subtle);border-top:var(--bw-hair) solid var(--border);
        font-size:12px;line-height:1.45;color:var(--text-muted);}
      .nk-diff__actions{display:flex;gap:8px;padding:12px 16px;border-top:var(--bw-hair) solid var(--border);}
      `;
      function useCSS() {
        React.useEffect(() => {
          if (document.getElementById("nk-diff-css")) return;
          const s = document.createElement("style");
          s.id = "nk-diff-css";
          s.textContent = CSS;
          document.head.appendChild(s);
        }, []);
      }
      const PILL = {
        pending: {
          label: "Precisa de aprovação",
          cls: "nk-diff__pill--pending",
        },
        approved: {
          label: "Aprovado",
          cls: "nk-diff__pill--approved",
        },
        rejected: {
          label: "Recusado",
          cls: "nk-diff__pill--rejected",
        },
      };
      function ApprovalDiffCard({
        title = "Mudança proposta",
        sheet = "Gastos 2025",
        range,
        changes = [
          {
            field: "Categoria",
            before: "Sem categoria",
            after: "Alimentação",
          },
          {
            field: "Dono",
            after: "Compartilhado",
          },
        ],
        note = null,
        status = "pending",
        actions = null,
        className = "",
      }) {
        useCSS();
        const pill = PILL[status] ?? PILL.pending;
        return /*#__PURE__*/ React.createElement(
          "article",
          {
            className: ["nk-diff", className].filter(Boolean).join(" "),
            "aria-label": `${title} — ${pill.label}`,
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "nk-diff__head",
            },
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "nk-diff__mark",
                "aria-hidden": "true",
              },
              /*#__PURE__*/ React.createElement(
                "svg",
                {
                  width: "16",
                  height: "16",
                  viewBox: "0 0 24 24",
                  fill: "none",
                  stroke: "currentColor",
                  strokeWidth: "2.2",
                  strokeLinecap: "round",
                  strokeLinejoin: "round",
                },
                /*#__PURE__*/ React.createElement("path", {
                  d: "M4 4h16v16H4z",
                }),
                /*#__PURE__*/ React.createElement("path", {
                  d: "M4 9h16M9 9v11",
                }),
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "nk-diff__htxt",
              },
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "nk-diff__title",
                },
                title,
              ),
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "nk-diff__src",
                },
                /*#__PURE__*/ React.createElement("b", null, sheet),
                range
                  ? /*#__PURE__*/ React.createElement("span", null, "\xB7 ", range)
                  : null,
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: `nk-diff__pill ${pill.cls}`,
              },
              pill.label,
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "nk-diff__rows",
            },
            changes.map((c, i) =>
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "nk-diff__row",
                  key: `${c.field}:${c.before ?? ""}:${c.after}:${i}`,
                },
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "nk-diff__field",
                  },
                  c.field,
                ),
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "nk-diff__vals",
                  },
                  c.before != null && c.before !== ""
                    ? /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          className: "nk-diff__before",
                        },
                        c.before,
                      )
                    : null,
                  /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      className: "nk-diff__arrow",
                    },
                    "\u2192",
                  ),
                  /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      className: "nk-diff__after",
                    },
                    c.after,
                  ),
                ),
              ),
            ),
          ),
          note
            ? /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "nk-diff__note",
                },
                note,
              )
            : null,
          actions
            ? /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "nk-diff__actions",
                },
                actions,
              )
            : null,
        );
      }
      Object.assign(__ds_scope, { ApprovalDiffCard });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/finance/ApprovalDiffCard.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/finance/BalanceTrajectory.jsx
  try {
    (() => {
      const CSS = `
      .nk-btraj{position:relative;width:100%;}
      .nk-btraj svg{display:block;}
      .nk-btraj__line{
        stroke-dasharray:1;
        stroke-dashoffset:1;
        animation:nk-btraj-draw var(--dur-deliberate,480ms) var(--ease-entrance,cubic-bezier(0.16,1,0.3,1)) forwards;
      }
      @keyframes nk-btraj-draw{to{stroke-dashoffset:0;}}
      @media (prefers-reduced-motion:reduce){
        .nk-btraj__line{animation:none;stroke-dasharray:none;stroke-dashoffset:0;}
      }
      .nk-btraj__tip{
        position:absolute;
        top:4px;
        pointer-events:none;
        background:var(--surface-elevated);
        border:1px solid var(--border-strong);
        border-radius:var(--radius-sm);
        padding:5px 9px;
        display:flex;
        flex-direction:column;
        gap:1px;
        box-shadow:var(--shadow-2);
        white-space:nowrap;
        z-index:10;
      }
      .nk-btraj__tip-day{
        font-family:var(--font-sans);
        font-size:11px;
        font-weight:600;
        color:var(--text-muted);
        letter-spacing:var(--ls-label);
      }
      .nk-btraj__tip-val{
        font-family:var(--font-money);
        font-variant-numeric:tabular-nums;
        font-size:13px;
        font-weight:700;
        color:var(--text-strong);
      }
      `;
      function useCSS() {
        React.useEffect(() => {
          if (document.getElementById("nk-btraj-css")) return;
          const s = document.createElement("style");
          s.id = "nk-btraj-css";
          s.textContent = CSS;
          document.head.appendChild(s);
        }, []);
      }

      /* ── helpers ──────────────────────────────────────────────────────────────── */

      function fmtBRL(cents) {
        const abs = Math.abs(cents);
        const sign = cents < 0 ? "-" : "";
        const reais = Math.floor(abs / 100);
        const centavos = String(abs % 100).padStart(2, "0");
        const formatted = reais.toLocaleString("pt-BR");
        return `${sign}R$ ${formatted},${centavos}`;
      }
      function fmtCompact(cents) {
        const abs = Math.abs(cents);
        const sign = cents < 0 ? "-" : "";
        if (abs >= 100_000_00) return `${sign}R$ ${(abs / 100_000_00).toFixed(1)}M`;
        if (abs >= 1_000_00) return `${sign}R$ ${(abs / 1_000_00).toFixed(1)}mil`;
        return fmtBRL(cents);
      }
      function fmtDayMonth(dateStr) {
        if (!dateStr) return "";
        const [, m, d] = dateStr.split("-");
        const months = [
          "jan",
          "fev",
          "mar",
          "abr",
          "mai",
          "jun",
          "jul",
          "ago",
          "set",
          "out",
          "nov",
          "dez",
        ];
        return `${parseInt(d, 10)} ${months[parseInt(m, 10) - 1] || ""}`;
      }

      /* ── demo data ────────────────────────────────────────────────────────────── */

      function buildDemo() {
        const today = new Date();
        const pad = (n) => String(n).padStart(2, "0");
        const fmt = (d) =>
          `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
        const days = [];
        let bal = 820000; // R$ 8 200,00 in cents
        for (let i = 0; i < 30; i++) {
          const d = new Date(today);
          d.setDate(today.getDate() + i);
          // gentle decline with a dip
          bal -= Math.round(Math.random() * 3000 + 1000);
          if (i === 18) bal -= 50000; // big expense spike
          days.push({
            date: fmt(d),
            balance_cents: bal,
          });
        }
        return {
          daily: days,
          today: fmt(today),
        };
      }
      const DEMO = buildDemo();

      /* ── component ────────────────────────────────────────────────────────────── */

      function BalanceTrajectory({
        daily = DEMO.daily,
        today = DEMO.today,
        variant = "full",
      }) {
        useCSS();
        const compact = variant === "compact";
        const W = 1000;
        const H = compact ? 120 : 260;
        const padX = 8;
        const padTop = compact ? 22 : 16;
        const padBottom = compact ? 22 : 28;
        const fs = compact ? 13 : 12;
        const gid = `bt-area-${variant}`;
        const wrapRef = React.useRef(null);
        const [hover, setHover] = React.useState(null);
        const vals = daily.map((d) => d.balance_cents);
        const maxVal = Math.max(...vals, 0);
        const minVal = Math.min(...vals, 0);
        const range = maxVal - minVal || 1;
        const innerW = W - padX * 2;
        const innerH = H - padTop - padBottom;
        const xOf = (i) =>
          padX + (daily.length === 1 ? innerW / 2 : (i / (daily.length - 1)) * innerW);
        const yOf = (cents) => padTop + innerH - ((cents - minVal) / range) * innerH;
        const labelX = (i) => Math.max(padX + 18, Math.min(W - padX - 18, xOf(i)));
        const linePts = daily
          .map((d, i) => `${xOf(i)},${yOf(d.balance_cents)}`)
          .join(" ");
        const areaPath = `M ${xOf(0)},${yOf(minVal)} L ${linePts.replace(/ /g, " L ")} L ${xOf(daily.length - 1)},${yOf(minVal)} Z`;
        const zeroY = yOf(0);
        const todayIdx = daily.findIndex((d) => d.date === today);
        const minIdx = vals.indexOf(Math.min(...vals));
        const hasDeficit = minVal < 0;

        // Accessible summary for screen readers
        const todayBal = todayIdx >= 0 ? daily[todayIdx] : null;
        const minDay = daily[minIdx];
        const lastDay = daily[daily.length - 1];
        const ariaSummary = daily.length
          ? [
              "Trajetória do saldo projetado.",
              todayBal ? `Hoje: ${fmtBRL(todayBal.balance_cents)}.` : "",
              minDay
                ? `Menor saldo: ${fmtBRL(minDay.balance_cents)} em ${fmtDayMonth(minDay.date)}${hasDeficit ? " (fica negativo)" : ""}.`
                : "",
              lastDay ? `Fim do horizonte: ${fmtBRL(lastDay.balance_cents)}.` : "",
            ]
              .filter(Boolean)
              .join(" ")
          : "Sem dados de saldo projetado.";
        const onMove = (e) => {
          const rect = wrapRef.current?.getBoundingClientRect();
          if (!rect || rect.width === 0) return;
          const frac = (e.clientX - rect.left) / rect.width;
          const i = Math.max(
            0,
            Math.min(daily.length - 1, Math.round(frac * (daily.length - 1))),
          );
          setHover(i);
        };
        const hovered = hover != null ? daily[hover] : null;
        const hoverFrac = hover != null ? xOf(hover) / W : 0;
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            ref: wrapRef,
            className: "nk-btraj",
            onMouseMove: onMove,
            onMouseLeave: () => setHover(null),
          },
          /*#__PURE__*/ React.createElement(
            "svg",
            {
              viewBox: `0 0 ${W} ${H}`,
              width: "100%",
              preserveAspectRatio: compact ? "none" : "xMidYMid meet",
              role: "img",
              "aria-label": ariaSummary,
              style: {
                display: "block",
                height: compact ? H : undefined,
              },
            },
            /*#__PURE__*/ React.createElement(
              "defs",
              null,
              /*#__PURE__*/ React.createElement(
                "linearGradient",
                {
                  id: gid,
                  x1: "0",
                  y1: "0",
                  x2: "0",
                  y2: "1",
                },
                /*#__PURE__*/ React.createElement("stop", {
                  offset: "0%",
                  stopColor: "var(--primary)",
                  stopOpacity: "0.28",
                }),
                /*#__PURE__*/ React.createElement("stop", {
                  offset: "100%",
                  stopColor: "var(--primary)",
                  stopOpacity: "0.02",
                }),
              ),
            ),
            hasDeficit &&
              /*#__PURE__*/ React.createElement(
                React.Fragment,
                null,
                /*#__PURE__*/ React.createElement("line", {
                  x1: padX,
                  x2: W - padX,
                  y1: zeroY,
                  y2: zeroY,
                  stroke: "var(--danger-400)",
                  strokeWidth: "1",
                  strokeDasharray: "3 4",
                  opacity: "0.7",
                }),
                !compact &&
                  /*#__PURE__*/ React.createElement(
                    "text",
                    {
                      x: W - padX,
                      y: zeroY - 5,
                      textAnchor: "end",
                      fontSize: fs,
                      fill: "var(--danger-400)",
                    },
                    "R$ 0",
                  ),
              ),
            /*#__PURE__*/ React.createElement("path", {
              d: areaPath,
              fill: `url(#${gid})`,
            }),
            /*#__PURE__*/ React.createElement("polyline", {
              className: "nk-btraj__line",
              pathLength: 1,
              points: linePts,
              fill: "none",
              stroke: "var(--primary)",
              strokeWidth: compact ? 2 : 2.5,
              strokeLinecap: "round",
              strokeLinejoin: "round",
            }),
            hovered &&
              /*#__PURE__*/ React.createElement(
                "g",
                {
                  "aria-hidden": "true",
                },
                /*#__PURE__*/ React.createElement("line", {
                  x1: xOf(hover),
                  x2: xOf(hover),
                  y1: padTop,
                  y2: H - padBottom,
                  stroke: "var(--border-strong)",
                  strokeWidth: "1",
                }),
                /*#__PURE__*/ React.createElement("circle", {
                  cx: xOf(hover),
                  cy: yOf(hovered.balance_cents),
                  r: compact ? 3.5 : 4,
                  fill: "var(--primary)",
                  stroke: "var(--surface)",
                  strokeWidth: "2",
                }),
              ),
            todayIdx >= 0 &&
              /*#__PURE__*/ React.createElement(
                "g",
                null,
                /*#__PURE__*/ React.createElement("circle", {
                  cx: xOf(todayIdx),
                  cy: yOf(daily[todayIdx].balance_cents),
                  r: compact ? 3.5 : 4,
                  fill: "var(--primary)",
                  stroke: "var(--surface)",
                  strokeWidth: "2",
                }),
                !compact &&
                  /*#__PURE__*/ React.createElement(
                    "text",
                    {
                      x: labelX(todayIdx),
                      y: H - 9,
                      textAnchor: "middle",
                      fontSize: fs,
                      fontWeight: "600",
                      fill: "var(--text-muted)",
                    },
                    "hoje",
                  ),
              ),
            minIdx >= 0 &&
              minIdx !== todayIdx &&
              /*#__PURE__*/ React.createElement(
                "g",
                null,
                /*#__PURE__*/ React.createElement("circle", {
                  cx: xOf(minIdx),
                  cy: yOf(vals[minIdx]),
                  r: compact ? 3 : 3.5,
                  fill: hasDeficit ? "var(--danger-400)" : "var(--text-faint)",
                }),
                /*#__PURE__*/ React.createElement(
                  "text",
                  {
                    x: labelX(minIdx),
                    y: yOf(vals[minIdx]) + (compact ? 16 : 18),
                    textAnchor: "middle",
                    fontSize: fs,
                    fontWeight: "600",
                    fill: hasDeficit ? "var(--danger-400)" : "var(--text-muted)",
                  },
                  fmtCompact(vals[minIdx]),
                ),
              ),
            !compact &&
              /*#__PURE__*/ React.createElement(
                "text",
                {
                  x: xOf(0),
                  y: yOf(maxVal) - 8,
                  textAnchor: "start",
                  fontSize: fs,
                  fontWeight: "600",
                  fill: "var(--text-muted)",
                },
                fmtCompact(maxVal),
              ),
          ),
          hovered &&
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "nk-btraj__tip",
                "aria-hidden": "true",
                style: {
                  left: `${hoverFrac * 100}%`,
                  transform: `translateX(${hoverFrac > 0.85 ? "-100%" : hoverFrac < 0.15 ? "0" : "-50%"})`,
                },
              },
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "nk-btraj__tip-day",
                },
                fmtDayMonth(hovered.date),
              ),
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "nk-btraj__tip-val",
                },
                fmtBRL(hovered.balance_cents),
              ),
            ),
        );
      }
      Object.assign(__ds_scope, { BalanceTrajectory });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/finance/BalanceTrajectory.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/finance/HealthBadge.jsx
  try {
    (() => {
      const TONE = {
        strong: {
          bg: "var(--success-tint)",
          border: "color-mix(in srgb, var(--success-400) 25%, transparent)",
          color: "var(--success-400)",
        },
        steady: {
          bg: "var(--primary-quiet)",
          border: "color-mix(in srgb, var(--primary) 22%, transparent)",
          color: "var(--primary)",
        },
        watch: {
          bg: "var(--warning-tint)",
          border: "color-mix(in srgb, var(--warning-400) 25%, transparent)",
          color: "var(--warning-400)",
        },
        risk: {
          bg: "var(--danger-tint)",
          border: "color-mix(in srgb, var(--danger-400) 25%, transparent)",
          color: "var(--danger-400)",
        },
      };
      const DEFAULT_LABEL = {
        strong: "Forte",
        steady: "Estável",
        watch: "Atenção",
        risk: "Em risco",
      };
      const DEFAULT_SCORE = {
        strong: 92,
        steady: 74,
        watch: 48,
        risk: 24,
      };
      function HealthBadge({
        level = "steady",
        label,
        score,
        sublabel = "",
        size = "md",
        className = "",
      }) {
        const tone = TONE[level];
        const text = label ?? DEFAULT_LABEL[level];
        const pct = score ?? DEFAULT_SCORE[level];
        const dim = size === "lg" ? 34 : 24;
        const r = size === "lg" ? 15 : 10;
        const c = 2 * Math.PI * r;
        const cx = dim / 2;
        const badgeStyle = {
          display: "inline-flex",
          alignItems: "center",
          gap: "10px",
          padding: size === "lg" ? "10px 18px 10px 12px" : "7px 13px 7px 9px",
          borderRadius: "var(--radius-pill)",
          fontFamily: "var(--font-sans)",
          lineHeight: 1,
          border: `1px solid ${tone.border}`,
          background: tone.bg,
          color: tone.color,
        };
        const ringStyle = {
          flex: "none",
          transform: "rotate(-90deg)",
        };
        const progressStyle = {
          transition: "stroke-dashoffset var(--dur-slow) var(--ease-entrance)",
        };
        const labelStyle = {
          fontSize: size === "lg" ? "var(--fs-title)" : "var(--fs-sm)",
          fontWeight: "var(--fw-bold)",
          letterSpacing: "-0.005em",
        };
        const sublabelStyle = {
          fontSize: "var(--fs-micro)",
          fontWeight: "var(--fw-medium)",
          opacity: 0.8,
        };
        return /*#__PURE__*/ React.createElement(
          "span",
          {
            className: className,
            style: badgeStyle,
          },
          /*#__PURE__*/ React.createElement(
            "svg",
            {
              "aria-hidden": "true",
              width: dim,
              height: dim,
              viewBox: `0 0 ${dim} ${dim}`,
              style: ringStyle,
            },
            /*#__PURE__*/ React.createElement("circle", {
              cx: cx,
              cy: cx,
              r: r,
              fill: "none",
              stroke: "currentColor",
              strokeWidth: "3",
              opacity: "0.2",
            }),
            /*#__PURE__*/ React.createElement("circle", {
              cx: cx,
              cy: cx,
              r: r,
              fill: "none",
              stroke: "currentColor",
              strokeWidth: "3",
              strokeLinecap: "round",
              strokeDasharray: c,
              strokeDashoffset: c * (1 - pct / 100),
              style: progressStyle,
            }),
          ),
          /*#__PURE__*/ React.createElement(
            "span",
            {
              style: {
                display: "flex",
                flexDirection: "column",
                gap: "2px",
              },
            },
            /*#__PURE__*/ React.createElement(
              "span",
              {
                style: labelStyle,
              },
              text,
            ),
            sublabel
              ? /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    style: sublabelStyle,
                  },
                  sublabel,
                )
              : null,
          ),
        );
      }
      Object.assign(__ds_scope, { HealthBadge });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/finance/HealthBadge.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/finance/LineItemEditor.jsx
  try {
    (() => {
      // LineItemEditor — controlled editor for itemized transaction parts.
      // Each item: R$ <magnitude> - <description>. Total shown when ≥2 items exist.
      // Self-contained: no external imports, no fetch, no Tauri. Inline-style convention.

      // ---- helpers ----------------------------------------------------------------

      function parseBRLToCents(input) {
        const cleaned = input
          .replace(/[R$\s]/g, "")
          .replace(/\./g, "")
          .replace(",", ".");
        if (!cleaned || !/^-?\d+(\.\d+)?$/.test(cleaned)) return null;
        return Math.round(Number(cleaned) * 100);
      }
      function formatBRL(cents) {
        const neg = cents < 0;
        const v = Math.abs(cents) / 100;
        const s = v.toLocaleString("pt-BR", {
          minimumFractionDigits: 2,
          maximumFractionDigits: 2,
        });
        return (neg ? "−R$ " : "R$ ") + s;
      }

      // ---- style constants --------------------------------------------------------

      const FIELD_BASE = {
        height: "var(--hit-min)",
        padding: "0 var(--space-3)",
        background: "var(--bg-subtle)",
        border: "var(--bw-hair) solid var(--border-input)",
        borderRadius: "var(--radius-xs)",
        color: "var(--text)",
        fontFamily: "var(--font-sans)",
        fontSize: "var(--fs-body)",
        outline: "none",
        boxSizing: "border-box",
      };
      const ITEM_AMOUNT = {
        ...FIELD_BASE,
        width: 120,
        fontFamily: "var(--font-money)",
        fontVariantNumeric: "tabular-nums",
        flexShrink: 0,
      };
      const ITEM_DESC = {
        ...FIELD_BASE,
        flex: 1,
        minWidth: 0,
      };
      const ROW = {
        display: "flex",
        gap: "var(--space-2)",
        alignItems: "center",
      };
      const LIST = {
        display: "grid",
        gap: "var(--space-2)",
      };
      const REMOVE_BTN = {
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        width: "var(--hit-min)",
        height: "var(--hit-min)",
        flexShrink: 0,
        borderRadius: "var(--radius-xs)",
        border: "var(--bw-hair) solid var(--border)",
        background: "transparent",
        color: "var(--text-muted)",
        cursor: "pointer",
        fontSize: "var(--fs-body)",
        lineHeight: 1,
      };
      const ADD_BTN = {
        display: "inline-flex",
        alignItems: "center",
        gap: "var(--space-2)",
        height: "var(--hit-min)",
        padding: "0 var(--space-3)",
        borderRadius: "var(--radius-sm)",
        border: "var(--bw-hair) dashed var(--border)",
        background: "transparent",
        color: "var(--text)",
        cursor: "pointer",
        fontFamily: "var(--font-sans)",
        fontSize: "var(--fs-sm)",
        width: "fit-content",
      };
      const SECTION_LABEL = {
        display: "block",
        fontSize: "var(--fs-label)",
        fontWeight: "var(--fw-semibold)",
        letterSpacing: "var(--ls-label)",
        textTransform: "uppercase",
        color: "var(--text-muted)",
        marginBottom: "var(--space-2)",
      };
      const TOTAL_LINE = {
        display: "flex",
        justifyContent: "space-between",
        alignItems: "baseline",
        marginTop: "var(--space-1)",
        fontSize: "var(--fs-sm)",
        color: "var(--text-muted)",
      };
      const TOTAL_VALUE = {
        fontFamily: "var(--font-money)",
        fontVariantNumeric: "tabular-nums",
        fontWeight: "var(--fw-semibold)",
        color: "var(--text)",
      };

      // ---- default demo items (render nicely with no required props) ---------------

      const DEMO_ITEMS = [
        {
          amount_cents: 8500,
          description: "Supermercado Pão de Açúcar",
          position: 0,
        },
        {
          amount_cents: 3200,
          description: "Padaria da esquina",
          position: 1,
        },
      ];

      // ---- component --------------------------------------------------------------

      function LineItemEditor({ items: itemsProp, onChange, disabled = false }) {
        // Standalone / demo mode: manage state internally when no onChange is provided
        const isControlled = typeof onChange === "function";
        const [internalItems, setInternalItems] = React.useState(
          itemsProp !== undefined ? itemsProp : DEMO_ITEMS,
        );
        const items = isControlled
          ? itemsProp !== undefined
            ? itemsProp
            : []
          : internalItems;
        function emit(next) {
          if (isControlled) {
            onChange(next);
          } else {
            setInternalItems(next);
          }
        }

        // Raw amount text per row (buffered to avoid cursor fighting while typing)
        const [amountText, setAmountText] = React.useState({});
        function displayAmount(index, cents) {
          const buffered = amountText[index];
          if (buffered !== undefined) return buffered;
          return cents > 0 ? (cents / 100).toFixed(2).replace(".", ",") : "";
        }
        function addItem() {
          emit([
            ...items,
            {
              amount_cents: 0,
              description: "",
              position: items.length,
            },
          ]);
        }
        function removeItem(index) {
          setAmountText({});
          const next = [];
          for (let i = 0; i < items.length; i++) {
            if (i === index) continue;
            const it = items[i];
            if (it)
              next.push({
                ...it,
                position: next.length,
              });
          }
          emit(next);
        }
        function setAmount(index, raw) {
          setAmountText((prev) => ({
            ...prev,
            [index]: raw,
          }));
          const cents = parseBRLToCents(raw);
          emit(
            items.map((it, i) =>
              i === index
                ? {
                    ...it,
                    amount_cents: cents !== null ? cents : 0,
                  }
                : it,
            ),
          );
        }
        function setDescription(index, value) {
          emit(
            items.map((it, i) =>
              i === index
                ? {
                    ...it,
                    description: value,
                  }
                : it,
            ),
          );
        }
        const total = items.reduce((sum, it) => sum + it.amount_cents, 0);
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            style: {
              fontFamily: "var(--font-sans)",
            },
          },
          /*#__PURE__*/ React.createElement(
            "span",
            {
              style: SECTION_LABEL,
            },
            "Detalhar em partes",
          ),
          items.length > 0 &&
            /*#__PURE__*/ React.createElement(
              "div",
              {
                style: LIST,
              },
              items.map((it, i) =>
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    key: it.position,
                    style: ROW,
                  },
                  /*#__PURE__*/ React.createElement("input", {
                    "aria-label": `Valor do item ${i + 1}`,
                    inputMode: "decimal",
                    placeholder: "R$ 0,00",
                    value: displayAmount(i, it.amount_cents),
                    onChange: (e) => setAmount(i, e.target.value),
                    disabled: disabled,
                    style: ITEM_AMOUNT,
                  }),
                  /*#__PURE__*/ React.createElement("input", {
                    "aria-label": `Descrição do item ${i + 1}`,
                    placeholder: "Descri\xE7\xE3o da parte\u2026",
                    value: it.description,
                    onChange: (e) => setDescription(i, e.target.value),
                    disabled: disabled,
                    style: ITEM_DESC,
                  }),
                  /*#__PURE__*/ React.createElement(
                    "button",
                    {
                      type: "button",
                      "aria-label": `Remover item ${i + 1}`,
                      onClick: () => removeItem(i),
                      disabled: disabled,
                      style: disabled
                        ? {
                            ...REMOVE_BTN,
                            opacity: 0.4,
                            cursor: "not-allowed",
                          }
                        : REMOVE_BTN,
                    },
                    "\xD7",
                  ),
                ),
              ),
            ),
          /*#__PURE__*/ React.createElement(
            "button",
            {
              type: "button",
              onClick: addItem,
              disabled: disabled,
              style: {
                ...ADD_BTN,
                marginTop: items.length > 0 ? "var(--space-2)" : 0,
                ...(disabled
                  ? {
                      opacity: 0.4,
                      cursor: "not-allowed",
                    }
                  : {}),
              },
            },
            "+ Adicionar item",
          ),
          items.length >= 2 &&
            /*#__PURE__*/ React.createElement(
              "p",
              {
                style: {
                  ...TOTAL_LINE,
                  margin: "var(--space-1) 0 0",
                },
              },
              /*#__PURE__*/ React.createElement("span", null, "Total das partes"),
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  style: TOTAL_VALUE,
                },
                formatBRL(total),
              ),
            ),
        );
      }
      Object.assign(__ds_scope, { LineItemEditor });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/finance/LineItemEditor.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/finance/MetricTile.jsx
  try {
    (() => {
      const CSS = `
      .nk-tile{
        display:flex;flex-direction:column;gap:var(--space-2);
        padding:var(--space-6);
        background:var(--surface);
        border:var(--bw-hair) solid var(--border);
        border-radius:var(--radius-md);
        box-shadow:var(--elev-card);
        min-width:0;
      }
      .nk-tile__header{display:flex;align-items:center;justify-content:space-between;}
      .nk-tile__label{
        font-family:var(--font-sans);
        font-size:var(--fs-label);
        font-weight:var(--fw-medium);
        color:var(--text-faint);
        letter-spacing:var(--ls-label);
        text-transform:uppercase;
        margin:0;
      }
      .nk-tile__icon{color:var(--text-faint);flex:none;display:inline-flex;}
      .nk-tile__val{
        font-family:var(--font-money);
        font-size:var(--fs-money-xl);
        font-variant-numeric:tabular-nums;
        font-weight:var(--fw-semibold);
        line-height:var(--lh-tight);
        color:var(--text);
        margin:0;
      }
      .nk-tile__foot{
        display:flex;align-items:center;gap:var(--space-3);
        margin-top:var(--space-1);
      }
      .nk-tile__delta{
        display:inline-flex;align-items:center;gap:4px;
        font-size:var(--fs-sm);
        font-weight:var(--fw-semibold);
      }
      .nk-tile__delta--up{color:var(--money-pos);}
      .nk-tile__delta--down{color:var(--money-neg);}
      .nk-tile__delta--neutral{color:var(--text-muted);}
      .nk-tile__sub{font-size:var(--fs-sm);color:var(--text-muted);}
      `;
      function useCSS() {
        React.useEffect(() => {
          if (document.getElementById("nk-tile-css")) return;
          const s = document.createElement("style");
          s.id = "nk-tile-css";
          s.textContent = CSS;
          document.head.appendChild(s);
        }, []);
      }

      /* Inline SVG icons (24×24, strokeWidth 1.75, round caps) */
      function IconTrendingUp() {
        return /*#__PURE__*/ React.createElement(
          "svg",
          {
            width: "13",
            height: "13",
            viewBox: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            strokeWidth: "2",
            strokeLinecap: "round",
            strokeLinejoin: "round",
            "aria-hidden": "true",
          },
          /*#__PURE__*/ React.createElement("polyline", {
            points: "23 6 13.5 15.5 8.5 10.5 1 18",
          }),
          /*#__PURE__*/ React.createElement("polyline", {
            points: "17 6 23 6 23 12",
          }),
        );
      }
      function IconTrendingDown() {
        return /*#__PURE__*/ React.createElement(
          "svg",
          {
            width: "13",
            height: "13",
            viewBox: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            strokeWidth: "2",
            strokeLinecap: "round",
            strokeLinejoin: "round",
            "aria-hidden": "true",
          },
          /*#__PURE__*/ React.createElement("polyline", {
            points: "23 18 13.5 8.5 8.5 13.5 1 6",
          }),
          /*#__PURE__*/ React.createElement("polyline", {
            points: "17 18 23 18 23 12",
          }),
        );
      }
      function IconMinus() {
        return /*#__PURE__*/ React.createElement(
          "svg",
          {
            width: "13",
            height: "13",
            viewBox: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            strokeWidth: "2",
            strokeLinecap: "round",
            strokeLinejoin: "round",
            "aria-hidden": "true",
          },
          /*#__PURE__*/ React.createElement("line", {
            x1: "5",
            y1: "12",
            x2: "19",
            y2: "12",
          }),
        );
      }
      function MetricTile({
        label = "Saldo do mês",
        value = "R$ 4.820,00",
        icon = null,
        delta = null,
        deltaDir = "neutral",
        sublabel = "",
        spark = null,
        className = "",
      }) {
        useCSS();
        const deltaColor =
          deltaDir === "up"
            ? "var(--money-pos)"
            : deltaDir === "down"
              ? "var(--money-neg)"
              : "var(--text-muted)";
        const sparkPoints =
          spark && spark.length > 0
            ? spark
                .map((v, i) => {
                  const max = Math.max(...spark);
                  const min = Math.min(...spark);
                  const range = max - min || 1;
                  const x = i * 6 + 3;
                  const y = 26 - ((v - min) / range) * 22;
                  return `${x},${y}`;
                })
                .join(" ")
            : null;
        return /*#__PURE__*/ React.createElement(
          "article",
          {
            className: ["nk-tile", className].filter(Boolean).join(" "),
            "aria-label": label,
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "nk-tile__header",
            },
            /*#__PURE__*/ React.createElement(
              "p",
              {
                className: "nk-tile__label",
              },
              label,
            ),
            icon
              ? /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "nk-tile__icon",
                  },
                  icon,
                )
              : null,
          ),
          /*#__PURE__*/ React.createElement(
            "p",
            {
              className: "nk-tile__val",
            },
            value,
          ),
          delta || sublabel
            ? /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "nk-tile__foot",
                },
                delta
                  ? /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        className: `nk-tile__delta nk-tile__delta--${deltaDir}`,
                        style: {
                          color: deltaColor,
                        },
                      },
                      deltaDir === "up"
                        ? /*#__PURE__*/ React.createElement(IconTrendingUp, null)
                        : deltaDir === "down"
                          ? /*#__PURE__*/ React.createElement(IconTrendingDown, null)
                          : /*#__PURE__*/ React.createElement(IconMinus, null),
                      delta,
                    )
                  : null,
                sublabel
                  ? /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        className: "nk-tile__sub",
                      },
                      sublabel,
                    )
                  : null,
              )
            : null,
          sparkPoints
            ? /*#__PURE__*/ React.createElement(
                "svg",
                {
                  height: "28",
                  width: "100%",
                  viewBox: `0 0 ${spark.length * 6} 28`,
                  preserveAspectRatio: "none",
                  style: {
                    marginTop: "var(--space-2)",
                  },
                },
                /*#__PURE__*/ React.createElement("polyline", {
                  fill: "none",
                  stroke: "var(--primary)",
                  strokeWidth: "2",
                  strokeLinecap: "round",
                  strokeLinejoin: "round",
                  points: sparkPoints,
                }),
              )
            : null,
        );
      }
      Object.assign(__ds_scope, { MetricTile });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/finance/MetricTile.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/finance/Money.jsx
  try {
    (() => {
      // Money — valor monetário BRL em mono tabular, com sinal de menos real (−) e cor por sinal.
      // Inline-style only — sem hooks, sem CSS injection.

      /** Replica do formatBRL de src/lib/format.ts, sem dependências externas. */
      function formatBRL(cents, hideCents) {
        const neg = cents < 0;
        const v = Math.abs(cents) / 100;
        const s = v.toLocaleString("pt-BR", {
          minimumFractionDigits: hideCents ? 0 : 2,
          maximumFractionDigits: hideCents ? 0 : 2,
        });
        // Sinal de menos tipográfico (U+2212) + NBSP após R$
        return (neg ? "−R$ " : "R$ ") + s;
      }
      const SIZE_FS = {
        sm: "var(--fs-money-sm)",
        md: "var(--fs-money-md)",
        lg: "var(--fs-money-lg)",
        display: "var(--fs-money-xl)",
      };
      function Money({
        cents = -123456,
        size = "md",
        sign = "none",
        hideCents = false,
        ariaLabel,
        className = "",
      }) {
        const color =
          sign === "negative"
            ? "var(--money-neg)"
            : sign === "auto"
              ? cents < 0
                ? "var(--money-neg)"
                : cents > 0
                  ? "var(--money-pos)"
                  : "var(--money-neutral)"
              : undefined;
        const heavy = size === "lg" || size === "display";
        const label =
          ariaLabel ??
          (cents < 0 ? "negativo " : "") + formatBRL(Math.abs(cents), hideCents);
        return /*#__PURE__*/ React.createElement(
          "span",
          {
            className: className || undefined,
            "aria-label": label,
            style: {
              fontFamily: "var(--font-money)",
              fontVariantNumeric: "tabular-nums",
              fontWeight: heavy ? "var(--fw-bold)" : "var(--fw-semibold)",
              fontSize: SIZE_FS[size] || SIZE_FS.md,
              letterSpacing: size === "display" ? "-0.01em" : "0",
              whiteSpace: "nowrap",
              color,
            },
          },
          formatBRL(cents, hideCents),
        );
      }
      Object.assign(__ds_scope, { Money });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/finance/Money.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/finance/MovBadge.jsx
  try {
    (() => {
      // MovBadge — badge de tipo de movimento (os 5 pilares do método do Neko).
      // Inline-style convention (no CSS injection needed — all styles are object literals).
      // Accessible: o círculo decorativo é aria-hidden; o nome do tipo é sempre exposto via
      // sr-only span (quando showLabel=false) ou via label visível (quando showLabel=true).

      const KIND_META = {
        entrada: {
          token: "var(--type-entrada)",
          glyph: "E",
          name: "Entrada",
        },
        saida: {
          token: "var(--type-saida)",
          glyph: "S",
          name: "Saída",
        },
        diario: {
          token: "var(--type-diario)",
          glyph: "D",
          name: "Diário",
        },
        economia: {
          token: "var(--type-economia)",
          glyph: "E",
          name: "Economia",
        },
        cartao: {
          token: "var(--type-cartao)",
          glyph: "C",
          name: "Cartão",
        },
      };
      const SR_ONLY = {
        position: "absolute",
        width: 1,
        height: 1,
        padding: 0,
        margin: -1,
        overflow: "hidden",
        clipPath: "inset(50%)",
        whiteSpace: "nowrap",
        border: 0,
      };
      const GLYPH_BASE = {
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        borderRadius: "50%",
        color: "var(--text-on-primary)",
        fontWeight: "var(--fw-bold)",
        fontFamily: "var(--font-sans)",
        lineHeight: 1,
        flexShrink: 0,
      };
      function MovBadge({
        kind = "saida",
        showLabel = false,
        size = 18,
        className = "",
      }) {
        const meta = KIND_META[kind] || KIND_META.saida;
        const glyphStyle = {
          ...GLYPH_BASE,
          width: size,
          height: size,
          background: meta.token,
          fontSize: `${Math.round(size * 0.56)}px`,
        };
        return /*#__PURE__*/ React.createElement(
          "span",
          {
            className: className,
            style: {
              display: "inline-flex",
              alignItems: "center",
              gap: "6px",
              fontFamily: "var(--font-sans)",
            },
          },
          /*#__PURE__*/ React.createElement(
            "span",
            {
              "aria-hidden": "true",
              style: glyphStyle,
            },
            meta.glyph,
          ),
          showLabel
            ? /*#__PURE__*/ React.createElement(
                "span",
                {
                  style: {
                    fontSize: "var(--fs-sm)",
                    color: "var(--text)",
                  },
                },
                meta.name,
              )
            : /*#__PURE__*/ React.createElement(
                "span",
                {
                  style: SR_ONLY,
                },
                meta.name,
              ),
        );
      }
      Object.assign(__ds_scope, { MovBadge });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/finance/MovBadge.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/finance/OwnerChip.jsx
  try {
    (() => {
      // OwnerChip — who owns a transaction/account (personal / partner / shared).
      // Inline-style only; no CSS injection. Matches production OwnerChip.tsx exactly.

      const OWNERS = {
        personal: {
          label: "Eu",
          color: "var(--owner-personal)",
        },
        partner: {
          label: "Parceiro(a)",
          color: "var(--owner-partner)",
        },
        shared: {
          label: "Compartilhado",
          color: "var(--owner-shared)",
        },
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
      function OwnerChip({
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
        return /*#__PURE__*/ React.createElement(
          "span",
          {
            className: className,
            title: note ? `${label} · ${note}` : label,
            style: chipStyle,
          },
          avatar
            ? /*#__PURE__*/ React.createElement(
                "span",
                {
                  "aria-hidden": "true",
                  style: avatarStyle,
                },
                initials(label),
              )
            : /*#__PURE__*/ React.createElement("span", {
                "aria-hidden": "true",
                style: {
                  width: 7,
                  height: 7,
                  borderRadius: "50%",
                  flex: "none",
                  background: o.color,
                },
              }),
          label,
          note
            ? /*#__PURE__*/ React.createElement(
                "span",
                {
                  style: {
                    color: "var(--text-faint)",
                  },
                },
                note,
              )
            : null,
        );
      }
      Object.assign(__ds_scope, { OwnerChip });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/finance/OwnerChip.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/finance/PhaseBadge.jsx
  try {
    (() => {
      // PhaseBadge — método-adaptation phase (Mapear · Calibrar · Operar) with a
      // 3-segment progress indicator. Self-contained; inline-style convention (like Badge/MovBadge).

      const PHASES = [
        {
          key: "map",
          label: "Mapear",
        },
        {
          key: "calibrate",
          label: "Calibrar",
        },
        {
          key: "operate",
          label: "Operar",
        },
      ];
      const SR = {
        position: "absolute",
        width: 1,
        height: 1,
        padding: 0,
        margin: -1,
        overflow: "hidden",
        clip: "rect(0 0 0 0)",
        whiteSpace: "nowrap",
        border: 0,
      };
      const WRAP = {
        display: "inline-flex",
        alignItems: "center",
        gap: "7px",
        height: 22,
        padding: "0 10px",
        borderRadius: "var(--radius-pill)",
        background: "var(--bg-subtle)",
        border: "var(--bw-hair) solid var(--border)",
        fontSize: "var(--fs-micro)",
        fontWeight: "var(--fw-semibold)",
        letterSpacing: "var(--ls-label)",
        textTransform: "uppercase",
        color: "var(--text-muted)",
        whiteSpace: "nowrap",
      };
      function PhaseBadge({ phase = "calibrate" }) {
        const idx = Math.max(
          0,
          PHASES.findIndex((p) => p.key === phase),
        );
        const current = PHASES[idx] || PHASES[0];
        return /*#__PURE__*/ React.createElement(
          "span",
          {
            style: WRAP,
          },
          /*#__PURE__*/ React.createElement(
            "span",
            {
              style: SR,
            },
            `Fase de adaptação: ${current.label} (${idx + 1} de 3)`,
          ),
          /*#__PURE__*/ React.createElement(
            "span",
            {
              "aria-hidden": "true",
              style: {
                display: "inline-flex",
                gap: 2,
              },
            },
            PHASES.map((p, i) =>
              /*#__PURE__*/ React.createElement("span", {
                key: p.key,
                style: {
                  width: 9,
                  height: 4,
                  borderRadius: 1,
                  background: i <= idx ? "var(--primary)" : "var(--surface-2)",
                },
              }),
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "span",
            {
              "aria-hidden": "true",
            },
            current.label,
          ),
        );
      }
      Object.assign(__ds_scope, { PhaseBadge });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/finance/PhaseBadge.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/finance/ProvBadge.jsx
  try {
    (() => {
      // ProvBadge — proveniência de um lançamento (Da planilha · Do app · Previsto).
      // Ponto colorido + rótulo em badge pill. Inclui popover educativo inline
      // (sem dependência externa — a produção usa <InfoPopover>, aqui é recreação
      // self-contained). Cor nunca é sinal único: sempre acompanha a palavra.

      const CSS = `
      .nk-prov{position:relative;display:inline-flex;}
      .nk-prov__badge{display:inline-flex;align-items:center;gap:5px;height:20px;
        padding:0 8px 0 6px;border-radius:var(--radius-pill);
        background:var(--bg-subtle);border:var(--bw-hair) solid var(--border);
        font-size:var(--fs-micro);font-weight:var(--fw-medium);
        color:var(--text-muted);white-space:nowrap;cursor:default;
        font-family:var(--font-sans);}
      .nk-prov__dot{width:6px;height:6px;border-radius:50%;flex:none;}
      .nk-prov__tip{position:absolute;bottom:calc(100% + 6px);left:50%;
        transform:translateX(-50%);z-index:200;min-width:220px;max-width:260px;
        padding:10px 12px;border-radius:var(--radius-md);
        background:var(--surface-elevated,var(--surface));
        border:var(--bw-hair) solid var(--border-strong);
        box-shadow:var(--shadow-2,0 4px 16px rgba(0,0,0,.4));
        font-family:var(--font-sans);font-size:var(--fs-micro);
        line-height:1.5;color:var(--text-muted);pointer-events:none;}
      .nk-prov__tip-title{display:block;font-size:11.5px;font-weight:600;
        color:var(--text-strong);margin-bottom:4px;}
      @media (prefers-reduced-motion:no-preference){
        .nk-prov__tip{animation:nk-prov-fade 0.12s ease;}}
      @keyframes nk-prov-fade{from{opacity:0;transform:translateX(-50%) translateY(3px);}
        to{opacity:1;transform:translateX(-50%) translateY(0);}}
      `;
      function useCSS() {
        React.useEffect(() => {
          if (document.getElementById("nk-prov-css")) return;
          const s = document.createElement("style");
          s.id = "nk-prov-css";
          s.textContent = CSS;
          document.head.appendChild(s);
        }, []);
      }
      const PROV = {
        importado: {
          label: "Da planilha",
          dot: "var(--text-faint)",
          title: "Da planilha",
          body: "Você anotou na planilha e o app leu, igualzinho. Ainda não foi conferido com o banco.",
        },
        manual: {
          label: "Do app",
          dot: "var(--info-400)",
          title: "Do app",
          body: "Você lançou aqui no app. Ele também é gravado na planilha, valor a valor.",
        },
        projetado: {
          label: "Previsto",
          dot: "var(--secondary)",
          title: "Previsto",
          body: "Ainda não aconteceu. Pode ser um compromisso que você registrou ou uma projeção automática. Vira real quando o lançamento de verdade chega.",
        },
      };
      function ProvBadge({ provenance = "importado" }) {
        useCSS();
        const [open, setOpen] = React.useState(false);
        const p = PROV[provenance];
        if (!p) return null;
        return /*#__PURE__*/ React.createElement(
          "span",
          {
            className: "nk-prov",
            onMouseEnter: () => setOpen(true),
            onMouseLeave: () => setOpen(false),
            onFocus: () => setOpen(true),
            onBlur: () => setOpen(false),
          },
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: "nk-prov__badge",
              tabIndex: 0,
              role: "button",
              "aria-expanded": open,
              "aria-label": `Proveniência: ${p.title}. ${p.body}`,
            },
            /*#__PURE__*/ React.createElement("span", {
              "aria-hidden": "true",
              className: "nk-prov__dot",
              style: {
                background: p.dot,
              },
            }),
            p.label,
          ),
          open &&
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "nk-prov__tip",
                role: "tooltip",
                "aria-hidden": "true",
              },
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "nk-prov__tip-title",
                },
                p.title,
              ),
              p.body,
            ),
        );
      }
      Object.assign(__ds_scope, { ProvBadge });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/finance/ProvBadge.jsx",
      error: String((e && e.message) || e),
    });
  }

  // components/finance/TransactionRow.jsx
  try {
    (() => {
      function _extends() {
        return (
          (_extends = Object.assign
            ? Object.assign.bind()
            : function (n) {
                for (var e = 1; e < arguments.length; e++) {
                  var t = arguments[e];
                  for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]);
                }
                return n;
              }),
          _extends.apply(null, arguments)
        );
      }
      /**
       * TransactionRow — linha de lançamento fiel ao método: data, descrição, método, valor, procedência,
       * titular e nota. Quando o lançamento é um lump de fatura (Saída agregada), expande os itens da
       * nota da célula. Portado do production TransactionRow.tsx; inline-style convention (zero classes).
       */

      // ---------------------------------------------------------------------------
      // Helpers
      // ---------------------------------------------------------------------------

      /** Formata centavos BRL → "R$ 1.234,56" (− real, U+2212). */
      function formatBRL(cents) {
        const neg = cents < 0;
        const v = Math.abs(cents) / 100;
        const s = v.toLocaleString("pt-BR", {
          minimumFractionDigits: 2,
          maximumFractionDigits: 2,
        });
        return (neg ? "−R$ " : "R$ ") + s;
      }

      // ---------------------------------------------------------------------------
      // Provenance
      // ---------------------------------------------------------------------------

      const PROV = {
        importado: {
          label: "Da planilha",
          color: "var(--prov-imported)",
        },
        manual: {
          label: "Do app",
          color: "var(--prov-app)",
        },
        projetado: {
          label: "Previsto",
          color: "var(--prov-projected)",
        },
        conciliado: {
          label: "Conferido",
          color: "var(--prov-reconciled)",
        },
      };
      function ProvBadge({ provenance }) {
        if (!provenance) return null;
        const g = PROV[provenance];
        if (!g) return null;
        return /*#__PURE__*/ React.createElement(
          "span",
          {
            title: g.label,
            style: {
              display: "inline-flex",
              alignItems: "center",
              gap: "6px",
              fontSize: "var(--fs-micro)",
              fontWeight: "var(--fw-semibold)",
              color: "var(--text-muted)",
              whiteSpace: "nowrap",
            },
          },
          /*#__PURE__*/ React.createElement("span", {
            "aria-hidden": "true",
            style: {
              width: 7,
              height: 7,
              borderRadius: "50%",
              flex: "none",
              background: g.color,
            },
          }),
          g.label,
        );
      }

      // ---------------------------------------------------------------------------
      // Static style objects (defined outside the component to avoid re-creation)
      // ---------------------------------------------------------------------------

      const PASSTHROUGH_BADGE_STYLE = {
        fontSize: "var(--fs-label)",
        fontWeight: "var(--fw-bold)",
        textTransform: "uppercase",
        letterSpacing: "0.04em",
        color: "var(--info-400)",
        background: "var(--info-tint)",
        padding: "1px 6px",
        borderRadius: "4px",
        whiteSpace: "nowrap",
      };
      const LUMP_TOGGLE_BASE = {
        width: 18,
        height: 18,
        display: "grid",
        placeItems: "center",
        border: "none",
        background: "transparent",
        color: "var(--text-faint)",
        borderRadius: "4px",
        cursor: "pointer",
        flexShrink: 0,
        transition: "transform var(--dur-fast) var(--ease-standard)",
      };
      function moneyStyle(amount) {
        return {
          fontFamily: "var(--font-money)",
          fontVariantNumeric: "tabular-nums",
          fontWeight: "var(--fw-semibold)",
          fontSize: "var(--fs-money-sm)",
          textAlign: "right",
          whiteSpace: "nowrap",
          color: amount > 0 ? "var(--money-pos)" : "var(--text)",
        };
      }
      function lumpItemKey(it) {
        return `${it.what}:${it.amount}:${it.passthrough ? "repasse" : "normal"}`;
      }

      // ---------------------------------------------------------------------------
      // Main component
      // ---------------------------------------------------------------------------

      function TransactionRow({
        date = "21/06",
        desc = "Supermercado Central",
        amount = -38500,
        method = "Débito",
        provenance = "importado",
        owner = null,
        note = null,
        passthrough = false,
        future = false,
        lump = null,
        defaultOpen = false,
        selected = false,
        onClick = null,
        className = "",
      }) {
        const [open, setOpen] = React.useState(defaultOpen);
        const hasLump = Array.isArray(lump) && lump.length > 0;
        const toggleStyle = {
          ...LUMP_TOGGLE_BASE,
          transform: open ? "rotate(90deg)" : "none",
        };
        const rowInteractionProps = onClick
          ? {
              onClick,
              onKeyDown: (event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  onClick();
                }
              },
              role: "button",
              tabIndex: 0,
            }
          : {};
        const futureBackground = future
          ? "repeating-linear-gradient(135deg, transparent, transparent 9px, color-mix(in srgb, var(--warning-500) 5%, transparent) 9px, color-mix(in srgb, var(--warning-500) 5%, transparent) 18px)"
          : "transparent";
        const showMeta = provenance || owner || note;
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: className,
            style: {
              borderBottom: "var(--bw-hair) solid var(--border)",
              fontFamily: "var(--font-sans)",
              background: selected ? "var(--surface-selected)" : futureBackground,
              boxShadow: "none",
            },
          },
          /*#__PURE__*/ React.createElement(
            "div",
            _extends({}, rowInteractionProps, {
              style: {
                display: "grid",
                gridTemplateColumns: "58px 1fr auto auto",
                alignItems: "center",
                gap: "14px",
                padding: "12px 18px",
              },
            }),
            /*#__PURE__*/ React.createElement(
              "span",
              {
                style: {
                  fontSize: "var(--fs-sm)",
                  color: "var(--text-faint)",
                  fontFamily: "var(--font-money)",
                  whiteSpace: "nowrap",
                },
              },
              date,
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                style: {
                  minWidth: 0,
                  display: "flex",
                  flexDirection: "column",
                  gap: "4px",
                },
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  style: {
                    display: "flex",
                    alignItems: "center",
                    gap: "8px",
                  },
                },
                hasLump
                  ? /*#__PURE__*/ React.createElement(
                      "button",
                      {
                        type: "button",
                        "aria-expanded": open,
                        "aria-label": open ? "Fechar itens" : "Abrir itens",
                        onClick: (e) => {
                          e.stopPropagation();
                          setOpen((o) => !o);
                        },
                        style: toggleStyle,
                      },
                      "\u203A",
                    )
                  : /*#__PURE__*/ React.createElement("span", {
                      style: {
                        width: 18,
                        flexShrink: 0,
                      },
                    }),
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    style: {
                      fontSize: "var(--fs-body)",
                      color: "var(--text)",
                      overflowWrap: "anywhere",
                    },
                  },
                  desc,
                ),
                passthrough
                  ? /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        style: PASSTHROUGH_BADGE_STYLE,
                      },
                      "repasse",
                    )
                  : null,
              ),
              showMeta
                ? /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      style: {
                        display: "flex",
                        alignItems: "center",
                        gap: "10px",
                        flexWrap: "wrap",
                        paddingLeft: 26,
                      },
                    },
                    /*#__PURE__*/ React.createElement(ProvBadge, {
                      provenance: provenance,
                    }),
                    owner,
                    note
                      ? /*#__PURE__*/ React.createElement(
                          "span",
                          {
                            style: {
                              fontSize: "var(--fs-micro)",
                              color: "var(--text-faint)",
                              fontStyle: "italic",
                            },
                          },
                          `"${note}"`,
                        )
                      : null,
                  )
                : null,
            ),
            method
              ? /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    style: {
                      fontSize: "var(--fs-micro)",
                      color: "var(--text-muted)",
                      padding: "3px 9px",
                      border: "var(--bw-hair) solid var(--border)",
                      borderRadius: "var(--radius-pill)",
                      whiteSpace: "nowrap",
                    },
                  },
                  method,
                )
              : null,
            /*#__PURE__*/ React.createElement(
              "span",
              {
                style: {
                  ...moneyStyle(amount),
                  opacity: passthrough ? 0.55 : 1,
                },
              },
              formatBRL(amount),
            ),
          ),
          hasLump && open
            ? /*#__PURE__*/ React.createElement(
                "div",
                {
                  style: {
                    padding: "4px 18px 14px 76px",
                    background: "var(--bg-subtle)",
                    borderTop: "1px dashed var(--border)",
                  },
                },
                lump.map((it) =>
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      key: lumpItemKey(it),
                      style: {
                        display: "flex",
                        alignItems: "center",
                        gap: "10px",
                        padding: "7px 0",
                        borderBottom: "var(--bw-hair) solid var(--border)",
                        fontSize: "var(--fs-sm)",
                      },
                    },
                    /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        style: {
                          color: "var(--text-faint)",
                          fontFamily: "var(--font-money)",
                        },
                      },
                      "\u21B3",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        style: {
                          flex: 1,
                          color: "var(--text-muted)",
                          minWidth: 0,
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        },
                      },
                      it.what,
                    ),
                    it.owner || null,
                    /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        style: moneyStyle(it.amount),
                      },
                      formatBRL(it.amount),
                    ),
                  ),
                ),
                /*#__PURE__*/ React.createElement(
                  "p",
                  {
                    style: {
                      margin: "10px 0 0",
                      fontSize: "var(--fs-micro)",
                      color: "var(--text-faint)",
                    },
                  },
                  'Esse detalhe vem das notas da c\xE9lula da planilha. Cada item \xE9 preservado; nunca vira um "Sa\xEDda" gen\xE9rico.',
                ),
              )
            : null,
        );
      }
      Object.assign(__ds_scope, { TransactionRow });
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "components/finance/TransactionRow.jsx",
      error: String((e && e.message) || e),
    });
  }

  __ds_ns.ChatBubble = __ds_scope.ChatBubble;

  __ds_ns.Citation = __ds_scope.Citation;

  __ds_ns.EmptyState = __ds_scope.EmptyState;

  __ds_ns.MiaAvatar = __ds_scope.MiaAvatar;

  __ds_ns.Badge = __ds_scope.Badge;

  __ds_ns.Button = __ds_scope.Button;

  __ds_ns.Disclosure = __ds_scope.Disclosure;

  __ds_ns.InfoPopover = __ds_scope.InfoPopover;

  __ds_ns.Input = __ds_scope.Input;

  __ds_ns.MonthNav = __ds_scope.MonthNav;

  __ds_ns.NekoMark = __ds_scope.NekoMark;

  __ds_ns.SegmentedControl = __ds_scope.SegmentedControl;

  __ds_ns.Switch = __ds_scope.Switch;

  __ds_ns.ApprovalDiffCard = __ds_scope.ApprovalDiffCard;

  __ds_ns.BalanceTrajectory = __ds_scope.BalanceTrajectory;

  __ds_ns.HealthBadge = __ds_scope.HealthBadge;

  __ds_ns.LineItemEditor = __ds_scope.LineItemEditor;

  __ds_ns.MetricTile = __ds_scope.MetricTile;

  __ds_ns.Money = __ds_scope.Money;

  __ds_ns.MovBadge = __ds_scope.MovBadge;

  __ds_ns.OwnerChip = __ds_scope.OwnerChip;

  __ds_ns.PhaseBadge = __ds_scope.PhaseBadge;

  __ds_ns.ProvBadge = __ds_scope.ProvBadge;

  __ds_ns.TransactionRow = __ds_scope.TransactionRow;

  // ui_kits/ano-inteiro/YearGridScreen.jsx
  try {
    (() => {
      /* Neko Finance — Ano inteiro (YearGridScreen).
         Grade dia a dia para todos os 12 meses do ano: Data · Entrada · Saída · Diário · Saldo.
         A coluna Saldo usa o heatmap de cinco faixas canônicas (termômetro da planilha de referência).
         PT-BR copy · R$ em mono tabular · zero dependências externas.
         Expõe window.YearGridScreen. */

      const NS = window.NekoFinanceDesignSystem_9bd1cd;
      const { MonthNav, EmptyState } = NS;
      const Icon = window.Icon;

      /* ---- CSS (once-only) ---- */
      (function injectAnoInteiroCSS() {
        if (document.getElementById("ano-inteiro-css")) return;
        const s = document.createElement("style");
        s.id = "ano-inteiro-css";
        s.textContent = `
      /* Página */
      .yr { max-width: 1100px; margin: 0 auto; padding: var(--space-2); }

      /* Cabeçalho */
      .yr-header {
        display: flex; align-items: center; justify-content: space-between;
        gap: var(--space-4); margin-bottom: var(--space-6); flex-wrap: wrap;
      }
      .yr-title {
        font-size: var(--fs-h2); font-weight: var(--fw-bold);
        letter-spacing: var(--ls-tight); margin: 0; color: var(--text-strong);
      }
      .yr-subtitle {
        color: var(--text-muted); font-size: var(--fs-sm);
        margin: var(--space-1) 0 0;
      }

      /* Sections */
      .yr-sections { display: flex; flex-direction: column; gap: var(--space-6); }

      /* Título de seção (mês) */
      .yr-month-title {
        font-size: var(--fs-title); font-weight: var(--fw-bold);
        margin: 0 0 var(--space-3); color: var(--text-strong);
      }

      /* Card compartilhado */
      .yr-card {
        background: var(--surface);
        border: var(--bw-hair) solid var(--border);
        border-radius: var(--radius-md);
        box-shadow: var(--shadow-1);
      }
      .yr-card__body { padding: 0; }

      /* Scroll horizontal */
      .yr-scroll { overflow-x: auto; -webkit-overflow-scrolling: touch; }

      /* Tabela */
      .yr-table {
        width: 100%; border-collapse: collapse;
        font-size: var(--fs-sm); line-height: var(--lh-snug);
      }
      .yr-table thead th {
        padding: var(--space-2) var(--space-3);
        border-bottom: var(--bw-hair) solid var(--border);
        font-size: var(--fs-label); font-weight: var(--fw-semibold);
        letter-spacing: var(--ls-label); text-transform: uppercase;
        color: var(--text-muted); text-align: right; white-space: nowrap;
      }
      .yr-table thead th:first-child { text-align: left; }
      .yr-table tbody td {
        padding: var(--space-2) var(--space-3);
        border-bottom: var(--bw-hair) solid var(--border);
        color: var(--text); text-align: right; white-space: nowrap;
        font-family: var(--font-money); font-variant-numeric: tabular-nums;
      }
      .yr-table tbody td:first-child {
        font-family: var(--font-sans); text-align: left;
        font-size: var(--fs-sm); color: var(--text-muted);
      }
      .yr-table tbody tr:last-child td { border-bottom: none; }
      .yr-table tbody tr:hover td { background: var(--surface-hover); }
      .yr-td-dash { color: var(--text-faint); }
      .yr-td-saldo-empty { color: var(--text-faint); text-align: right; }

      /* Legenda do heatmap */
      .yr-legend {
        display: flex; align-items: center; gap: var(--space-5);
        padding: var(--space-3) var(--space-4);
        border-top: var(--bw-hair) solid var(--border);
        flex-wrap: wrap;
      }
      .yr-legend__label {
        font-size: var(--fs-micro); color: var(--text-faint);
        letter-spacing: var(--ls-label); text-transform: uppercase;
        margin-right: var(--space-1);
      }
      .yr-legend__item {
        display: flex; align-items: center; gap: var(--space-2);
        font-size: var(--fs-micro); color: var(--text-muted);
      }
      .yr-legend__swatch {
        width: 10px; height: 10px; border-radius: 2px; flex-shrink: 0;
      }

      @media (prefers-reduced-motion: reduce) {
        .yr-table tbody tr { transition: none; }
      }
      `;
        document.head.appendChild(s);
      })();

      /* ---- Helpers ---- */
      function fmtBRL(cents) {
        const abs = Math.abs(cents);
        const n = (abs / 100).toLocaleString("pt-BR", {
          minimumFractionDigits: 2,
          maximumFractionDigits: 2,
        });
        return "R$ " + n;
      }
      function fmtDayMonth(dateStr) {
        // dateStr = "2026-01-05" → "05/01"
        const parts = dateStr.split("-");
        return `${parts[2]}/${parts[1]}`;
      }

      /** Classifica o saldo (centavos) nas mesmas 5 faixas canônicas da planilha. */
      function saldoBand(cents) {
        if (cents < -50000) return "critical";
        if (cents < 0) return "negative";
        if (cents <= 100000) return "tight";
        if (cents <= 200000) return "ok";
        return "comfortable";
      }
      const BAND_FILL = {
        critical: "var(--saldo-band-critical-fill)",
        negative: "var(--saldo-band-negative-fill)",
        tight: "var(--saldo-band-tight-fill)",
        ok: "var(--saldo-band-ok-fill)",
        comfortable: "var(--saldo-band-comfortable-fill)",
      };
      const BAND_LABEL = {
        critical: "crítico",
        negative: "negativo",
        tight: "apertado",
        ok: "ok",
        comfortable: "folga",
      };

      /* ---- Dados de demonstração ---- */
      /**
       * Gera linhas diárias representativas para um mês.
       * Apenas dias com algum lançamento são exibidos (como na grade real).
       */
      function makeMonthData(year, month) {
        // Data de referência: hoje é 21/06/2026
        const today = new Date(2026, 5, 21); // mês 0-indexado
        const mDate = new Date(year, month - 1, 1);
        const isPast = mDate < new Date(2026, 5, 1);
        const isCurrent = month === 6 && year === 2026;
        const isFuture = mDate > new Date(2026, 5, 1);
        if (isFuture) return []; // meses futuros sem dados

        const DAYS_IN_MONTH = new Date(year, month, 0).getDate();

        // Padrões por mês — números fictícios mas realistas
        const monthPatterns = {
          1: {
            income: 850000,
            salary_day: 5,
            out_days: [8, 10, 15],
            daily_avg: 18000,
            start_bal: 312000,
          },
          2: {
            income: 850000,
            salary_day: 5,
            out_days: [7, 12, 14],
            daily_avg: 15000,
            start_bal: 218500,
          },
          3: {
            income: 850000,
            salary_day: 5,
            out_days: [8, 11, 15],
            daily_avg: 19000,
            start_bal: 174200,
          },
          4: {
            income: 850000,
            salary_day: 7,
            out_days: [9, 13, 16],
            daily_avg: 17000,
            start_bal: 156900,
          },
          5: {
            income: 850000,
            salary_day: 5,
            out_days: [8, 10, 14],
            daily_avg: 21000,
            start_bal: 241300,
          },
          6: {
            income: 850000,
            salary_day: 5,
            out_days: [10, 15],
            daily_avg: 14500,
            start_bal: 357800,
          },
        };
        const p = monthPatterns[month] || monthPatterns[1];
        const rows = [];
        let balance = p.start_bal;
        for (let d = 1; d <= DAYS_IN_MONTH; d++) {
          const dateStr = `${year}-${String(month).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
          const dayOfMonth = d;
          const currDate = new Date(year, month - 1, d);
          if (isCurrent && currDate > today) break; // mês corrente: só até hoje

          const income = dayOfMonth === p.salary_day ? p.income : 0;
          const fixed_out = p.out_days.includes(dayOfMonth)
            ? Math.round(80000 + Math.random() * 40000)
            : 0;
          // Dias úteis com diário (segunda a sexta)
          const dow = currDate.getDay();
          const isWorkday = dow >= 1 && dow <= 5;
          const daily_out =
            isWorkday && !p.out_days.includes(dayOfMonth)
              ? Math.round(p.daily_avg * (0.6 + Math.random() * 0.8))
              : 0;

          // Só emite linhas que têm algum dado
          if (!income && !fixed_out && !daily_out) continue;
          balance = balance + income - fixed_out - daily_out;
          rows.push({
            date: dateStr,
            income_cents: income,
            fixed_out_cents: fixed_out,
            daily_out_cents: daily_out,
            balance_cents: balance,
          });
        }
        return rows;
      }
      const MONTHS_PT = [
        "Janeiro",
        "Fevereiro",
        "Março",
        "Abril",
        "Maio",
        "Junho",
        "Julho",
        "Agosto",
        "Setembro",
        "Outubro",
        "Novembro",
        "Dezembro",
      ];

      /* ---- Sub-componentes ---- */

      /** Legenda do heatmap (exibida uma vez, dentro da seção de junho). */
      function SaldoLegend() {
        const items = [
          {
            band: "comfortable",
            label: "folga (> R$ 2.000)",
          },
          {
            band: "ok",
            label: "ok (R$ 1.000–2.000)",
          },
          {
            band: "tight",
            label: "apertado (R$ 0–1.000)",
          },
          {
            band: "negative",
            label: "negativo",
          },
          {
            band: "critical",
            label: "crítico (< −R$ 500)",
          },
        ];
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "yr-legend",
            "aria-label": "Legenda do heatmap de saldo",
          },
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: "yr-legend__label",
            },
            "Saldo",
          ),
          items.map((i) =>
            /*#__PURE__*/ React.createElement(
              "span",
              {
                key: i.band,
                className: "yr-legend__item",
              },
              /*#__PURE__*/ React.createElement("span", {
                className: "yr-legend__swatch",
                style: {
                  background: BAND_FILL[i.band],
                },
                "aria-hidden": "true",
              }),
              i.label,
            ),
          ),
        );
      }

      /** Tabela de um mês. */
      function MonthTable({ grid }) {
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "yr-scroll",
          },
          /*#__PURE__*/ React.createElement(
            "table",
            {
              className: "yr-table",
            },
            /*#__PURE__*/ React.createElement(
              "thead",
              null,
              /*#__PURE__*/ React.createElement(
                "tr",
                null,
                /*#__PURE__*/ React.createElement(
                  "th",
                  {
                    scope: "col",
                  },
                  "Data",
                ),
                /*#__PURE__*/ React.createElement(
                  "th",
                  {
                    scope: "col",
                  },
                  "Entrada",
                ),
                /*#__PURE__*/ React.createElement(
                  "th",
                  {
                    scope: "col",
                  },
                  "Sa\xEDda",
                ),
                /*#__PURE__*/ React.createElement(
                  "th",
                  {
                    scope: "col",
                  },
                  "Di\xE1rio",
                ),
                /*#__PURE__*/ React.createElement(
                  "th",
                  {
                    scope: "col",
                  },
                  "Saldo",
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "tbody",
              null,
              grid.map((d) => {
                const band =
                  d.balance_cents != null ? saldoBand(d.balance_cents) : null;
                return /*#__PURE__*/ React.createElement(
                  "tr",
                  {
                    key: d.date,
                  },
                  /*#__PURE__*/ React.createElement("td", null, fmtDayMonth(d.date)),
                  /*#__PURE__*/ React.createElement(
                    "td",
                    null,
                    d.income_cents
                      ? /*#__PURE__*/ React.createElement(
                          "span",
                          {
                            style: {
                              color: "var(--money-pos)",
                            },
                          },
                          fmtBRL(d.income_cents),
                        )
                      : /*#__PURE__*/ React.createElement(
                          "span",
                          {
                            className: "yr-td-dash",
                          },
                          "\u2014",
                        ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "td",
                    null,
                    d.fixed_out_cents
                      ? fmtBRL(d.fixed_out_cents)
                      : /*#__PURE__*/ React.createElement(
                          "span",
                          {
                            className: "yr-td-dash",
                          },
                          "\u2014",
                        ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "td",
                    null,
                    d.daily_out_cents
                      ? fmtBRL(d.daily_out_cents)
                      : /*#__PURE__*/ React.createElement(
                          "span",
                          {
                            className: "yr-td-dash",
                          },
                          "\u2014",
                        ),
                  ),
                  d.balance_cents == null
                    ? /*#__PURE__*/ React.createElement(
                        "td",
                        {
                          className: "yr-td-saldo-empty",
                        },
                        "\u2014",
                      )
                    : /*#__PURE__*/ React.createElement(
                        "td",
                        {
                          style: {
                            textAlign: "right",
                            background: BAND_FILL[band],
                            color: "var(--text)",
                          },
                          title: `Saldo ${BAND_LABEL[band]}`,
                        },
                        fmtBRL(d.balance_cents),
                      ),
                );
              }),
            ),
          ),
        );
      }

      /** Seção de um mês: título + card com tabela ou estado vazio. */
      function MonthSection({ label, monthNum, grid, showLegend }) {
        const hasData = grid.length > 0;
        return /*#__PURE__*/ React.createElement(
          "section",
          {
            "aria-label": label,
          },
          /*#__PURE__*/ React.createElement(
            "h2",
            {
              className: "yr-month-title",
            },
            label,
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "yr-card",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "yr-card__body",
              },
              !hasData
                ? /*#__PURE__*/ React.createElement(EmptyState, {
                    variant: "empty",
                    title: "Sem lan\xE7amentos",
                    description: "Nenhum dado importado para este m\xEAs.",
                  })
                : /*#__PURE__*/ React.createElement(MonthTable, {
                    grid: grid,
                  }),
              showLegend &&
                hasData &&
                /*#__PURE__*/ React.createElement(SaldoLegend, null),
            ),
          ),
        );
      }

      /* ---- Tela completa ---- */
      function YearGridScreen(props) {
        const THIS_YEAR = 2026;
        const [year, setYear] = React.useState(THIS_YEAR);

        // 12 grids gerados a partir dos dados de demo
        const grids = MONTHS_PT.map((label, idx) => ({
          month: idx + 1,
          label,
          data: makeMonthData(year, idx + 1),
        }));

        // Exibe a legenda do heatmap no primeiro mês que tenha dados (para não repetir)
        let legendShown = false;
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "yr",
          },
          /*#__PURE__*/ React.createElement(
            "header",
            {
              className: "yr-header",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              null,
              /*#__PURE__*/ React.createElement(
                "h1",
                {
                  className: "yr-title",
                },
                "Ano inteiro",
              ),
              /*#__PURE__*/ React.createElement(
                "p",
                {
                  className: "yr-subtitle",
                },
                "Grade Data \xB7 Entrada \xB7 Sa\xEDda \xB7 Di\xE1rio \xB7 Saldo para cada m\xEAs de ",
                year,
                ".",
              ),
            ),
            /*#__PURE__*/ React.createElement(MonthNav, {
              label: String(year),
              onPrev: () => setYear((y) => y - 1),
              onNext: () => setYear((y) => y + 1),
              onToday: () => setYear(THIS_YEAR),
              atToday: year === THIS_YEAR,
              prevLabel: "Ano anterior",
              nextLabel: "Pr\xF3ximo ano",
            }),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "yr-sections",
            },
            grids.map((g) => {
              const showLegend = !legendShown && g.data.length > 0;
              if (showLegend) legendShown = true;
              return /*#__PURE__*/ React.createElement(MonthSection, {
                key: g.month,
                label: g.label,
                monthNum: g.month,
                grid: g.data,
                showLegend: showLegend,
              });
            }),
          ),
        );
      }
      window.YearGridScreen = YearGridScreen;
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "ui_kits/ano-inteiro/YearGridScreen.jsx",
      error: String((e && e.message) || e),
    });
  }

  // ui_kits/anuais/AnnualScreen.jsx
  try {
    (() => {
      /* Neko Finance — Visão anual (anuais).
         Entradas, economia e métricas de todos os meses do ano em uma tabela.
         Inclui sparkline de Economizado% com faixa-meta 20–30% sombreada.
         PT-BR copy · R$ em mono tabular · zero dependências externas.
         Expõe window.AnnualScreen. */

      const NS = window.NekoFinanceDesignSystem_9bd1cd;
      const { Money, MonthNav, InfoPopover } = NS;
      const Icon = window.Icon;

      /* ---- CSS (once-only) ---- */
      (function injectAnuaisCSS() {
        if (document.getElementById("anuais-css")) return;
        const s = document.createElement("style");
        s.id = "anuais-css";
        s.textContent = `
      /* Layout */
      .an { max-width: 860px; margin: 0 auto; padding: var(--space-2); }

      /* Cabeçalho */
      .an-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-4);
        margin-bottom: var(--space-6);
        flex-wrap: wrap;
      }
      .an-title {
        font-size: var(--fs-h2);
        font-weight: var(--fw-bold);
        letter-spacing: var(--ls-tight);
        margin: 0;
        color: var(--text-strong);
      }
      .an-subtitle {
        color: var(--text-muted);
        font-size: var(--fs-sm);
        margin: var(--space-1) 0 0;
        line-height: var(--lh-normal);
      }

      /* Card */
      .an-card {
        background: var(--surface);
        border: var(--bw-hair) solid var(--border);
        border-radius: var(--radius-md);
        box-shadow: var(--shadow-1);
      }
      .an-card__body {
        padding: var(--space-4) var(--space-6) var(--space-6);
      }

      /* Sparkline */
      .an-spark {
        margin-bottom: var(--space-6);
      }
      .an-spark__bars {
        display: flex;
        gap: 4px;
        align-items: flex-end;
        height: 56px;
        position: relative;
      }
      .an-spark__band {
        position: absolute;
        left: 0;
        right: 0;
        background: var(--success-tint);
        border-radius: 2px;
        pointer-events: none;
      }
      .an-spark__bar {
        flex: 1;
        border-radius: 2px 2px 0 0;
        position: relative;
        z-index: 1;
        transition: opacity 0.15s ease;
      }
      @media (prefers-reduced-motion: reduce) {
        .an-spark__bar { transition: none; }
      }
      .an-spark__bar:hover { opacity: 0.8; }
      .an-spark__legend {
        display: flex;
        flex-wrap: wrap;
        gap: var(--space-3);
        align-items: center;
        margin-top: var(--space-2);
        font-size: var(--fs-micro);
        color: var(--text-faint);
      }
      .an-spark__dot {
        display: inline-flex;
        align-items: center;
        gap: 4px;
      }
      .an-spark__swatch {
        width: 9px;
        height: 9px;
        border-radius: 2px;
        flex-shrink: 0;
      }

      /* Aviso sem Economia */
      .an-no-economia {
        margin: 0 0 var(--space-6);
        font-size: var(--fs-sm);
        color: var(--text-muted);
        line-height: var(--lh-normal);
      }

      /* Tabela */
      .an-scroll { overflow-x: auto; -webkit-overflow-scrolling: touch; }
      .an-table {
        width: 100%;
        border-collapse: collapse;
        font-variant-numeric: tabular-nums;
      }
      .an-table th {
        text-align: right;
        font-size: var(--fs-label);
        font-weight: var(--fw-semibold);
        letter-spacing: var(--ls-label);
        text-transform: uppercase;
        color: var(--text-muted);
        padding: var(--space-3) var(--space-4);
        white-space: nowrap;
      }
      .an-table th.col-mes { text-align: left; }
      .an-table th:first-child { text-align: left; }
      .an-thead-row {
        border-bottom: var(--bw-hair) solid var(--border);
      }
      .an-table td {
        text-align: right;
        padding: var(--space-3) var(--space-4);
        font-size: var(--fs-sm);
        white-space: nowrap;
      }
      .an-table td.col-mes {
        text-align: left;
        font-weight: var(--fw-semibold);
        color: var(--text);
      }
      .an-tbody-row {
        border-bottom: var(--bw-hair) solid var(--border);
      }
      .an-tbody-row.is-empty { opacity: 0.45; }
      .an-pct {
        font-family: var(--font-money);
        font-variant-numeric: tabular-nums;
      }
      .an-tfoot-row {
        border-top: var(--bw-strong) solid var(--border-strong);
        font-weight: var(--fw-bold);
      }
      .an-tfoot-row td, .an-tfoot-row th {
        padding: var(--space-3) var(--space-4);
        font-size: var(--fs-sm);
        white-space: nowrap;
      }
      .an-tfoot-row th {
        text-align: left;
        text-transform: uppercase;
        letter-spacing: var(--ls-label);
        font-size: var(--fs-label);
        color: var(--text);
      }
      .an-tfoot-row td {
        text-align: right;
      }
      `;
        document.head.appendChild(s);
      })();

      /* ---- Dados de demonstração ---- */
      const MONTHS_PT = [
        "Jan",
        "Fev",
        "Mar",
        "Abr",
        "Mai",
        "Jun",
        "Jul",
        "Ago",
        "Set",
        "Out",
        "Nov",
        "Dez",
      ];

      /* Ano de demonstração: 2025. Meses Jan–Jun com dados; Jul–Dez vazios (ano passado). */
      const DEMO_MONTHS = [
        {
          month: 1,
          income_cents: 850000,
          economia_cents: 192000,
          savings_rate_bps: 2259,
          performance_cents: 210000,
          cost_of_living_cents: 640000,
          real_daily_avg_cents: 19200,
        },
        {
          month: 2,
          income_cents: 850000,
          economia_cents: 212000,
          savings_rate_bps: 2494,
          performance_cents: 230000,
          cost_of_living_cents: 620000,
          real_daily_avg_cents: 20700,
        },
        {
          month: 3,
          income_cents: 850000,
          economia_cents: 152000,
          savings_rate_bps: 1788,
          performance_cents: 169000,
          cost_of_living_cents: 681000,
          real_daily_avg_cents: 18900,
        },
        {
          month: 4,
          income_cents: 850000,
          economia_cents: 245000,
          savings_rate_bps: 2882,
          performance_cents: 258000,
          cost_of_living_cents: 592000,
          real_daily_avg_cents: 17400,
        },
        {
          month: 5,
          income_cents: 1020000,
          economia_cents: 310000,
          savings_rate_bps: 3039,
          performance_cents: 328000,
          cost_of_living_cents: 692000,
          real_daily_avg_cents: 21800,
        },
        {
          month: 6,
          income_cents: 850000,
          economia_cents: 171000,
          savings_rate_bps: 2012,
          performance_cents: 185000,
          cost_of_living_cents: 665000,
          real_daily_avg_cents: 22100,
        },
        {
          month: 7,
          income_cents: 0,
          economia_cents: 0,
          savings_rate_bps: 0,
          performance_cents: 0,
          cost_of_living_cents: 0,
          real_daily_avg_cents: 0,
        },
        {
          month: 8,
          income_cents: 0,
          economia_cents: 0,
          savings_rate_bps: 0,
          performance_cents: 0,
          cost_of_living_cents: 0,
          real_daily_avg_cents: 0,
        },
        {
          month: 9,
          income_cents: 0,
          economia_cents: 0,
          savings_rate_bps: 0,
          performance_cents: 0,
          cost_of_living_cents: 0,
          real_daily_avg_cents: 0,
        },
        {
          month: 10,
          income_cents: 0,
          economia_cents: 0,
          savings_rate_bps: 0,
          performance_cents: 0,
          cost_of_living_cents: 0,
          real_daily_avg_cents: 0,
        },
        {
          month: 11,
          income_cents: 0,
          economia_cents: 0,
          savings_rate_bps: 0,
          performance_cents: 0,
          cost_of_living_cents: 0,
          real_daily_avg_cents: 0,
        },
        {
          month: 12,
          income_cents: 0,
          economia_cents: 0,
          savings_rate_bps: 0,
          performance_cents: 0,
          cost_of_living_cents: 0,
          real_daily_avg_cents: 0,
        },
      ];

      /* ---- Sub-componentes ---- */

      /** Item da legenda do sparkline: amostra de cor + rótulo. */
      function LegendDot({ color, label }) {
        return /*#__PURE__*/ React.createElement(
          "span",
          {
            className: "an-spark__dot",
          },
          /*#__PURE__*/ React.createElement("span", {
            "aria-hidden": "true",
            className: "an-spark__swatch",
            style: {
              background: color,
            },
          }),
          label,
        );
      }

      /** Mini-barras de Economizado% por mês, com faixa-meta 20–30% sombreada. */
      function EconomizadoSparkline({ months }) {
        const data = months.map((m) => ({
          pct: m.savings_rate_bps / 100,
          empty: m.income_cents === 0 && m.cost_of_living_cents === 0,
          label: MONTHS_PT[m.month - 1],
        }));
        const maxPct = Math.max(40, ...data.map((d) => d.pct));
        const H = 56;
        const bandTop = ((maxPct - 30) / maxPct) * H;
        const bandHeight = (10 / maxPct) * H; /* faixa 20–30% */

        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "an-spark",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              role: "img",
              "aria-label":
                "Tend\xEAncia de Economizado% por m\xEAs. Faixa-meta de 20 a 30% sombreada.",
              className: "an-spark__bars",
            },
            /*#__PURE__*/ React.createElement("span", {
              "aria-hidden": "true",
              className: "an-spark__band",
              style: {
                top: bandTop,
                height: bandHeight,
              },
            }),
            data.map((d, i) => {
              const h = d.empty ? 2 : Math.max(2, (d.pct / maxPct) * H);
              const color = d.empty
                ? "var(--border)"
                : d.pct > 30
                  ? "var(--primary)"
                  : d.pct >= 20
                    ? "var(--success-400)"
                    : "var(--warning-400)";
              return /*#__PURE__*/ React.createElement("span", {
                key: i,
                className: "an-spark__bar",
                title: `${d.label}: ${d.empty ? "—" : `${d.pct.toFixed(0)}%`}`,
                style: {
                  height: h,
                  background: color,
                },
              });
            }),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "an-spark__legend",
            },
            /*#__PURE__*/ React.createElement("span", null, "Economizado% por m\xEAs:"),
            /*#__PURE__*/ React.createElement(LegendDot, {
              color: "var(--success-tint)",
              label: "meta 20\u201330%",
            }),
            /*#__PURE__*/ React.createElement(LegendDot, {
              color: "var(--success-400)",
              label: "dentro",
            }),
            /*#__PURE__*/ React.createElement(LegendDot, {
              color: "var(--warning-400)",
              label: "abaixo",
            }),
            /*#__PURE__*/ React.createElement(LegendDot, {
              color: "var(--primary)",
              label: "acima",
            }),
          ),
        );
      }

      /* ---- Tela principal ---- */
      function AnnualScreen(props) {
        const [year, setYear] = React.useState(2025);
        const thisYear = 2026;
        const months = DEMO_MONTHS;

        /* Totais do ano — espelha a lógica de production */
        const totals = months.reduce(
          (a, m) => ({
            performance: a.performance + m.performance_cents,
            cost: a.cost + m.cost_of_living_cents,
            income: a.income + m.income_cents,
            economia: a.economia + m.economia_cents,
          }),
          {
            performance: 0,
            cost: 0,
            income: 0,
            economia: 0,
          },
        );

        /* Economizado% anual = ΣEconomia / ΣEntradas — NÃO média das taxas mensais */
        const annualSavingsPct =
          totals.income > 0 ? Math.round((totals.economia / totals.income) * 100) : 0;
        const hasYearData = months.some(
          (m) => m.income_cents !== 0 || m.cost_of_living_cents !== 0,
        );
        const hasEconomia = months.some((m) => m.economia_cents !== 0);

        /* 3 estados de cor: acima de 30% jade, dentro 20–30% verde, abaixo âmbar */
        const savingsColor =
          annualSavingsPct > 30
            ? "var(--primary)"
            : annualSavingsPct >= 20
              ? "var(--success-400)"
              : "var(--warning-400)";
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "an",
          },
          /*#__PURE__*/ React.createElement(
            "header",
            {
              className: "an-header",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              null,
              /*#__PURE__*/ React.createElement(
                "h1",
                {
                  className: "an-title",
                },
                "Vis\xE3o anual",
              ),
              /*#__PURE__*/ React.createElement(
                "p",
                {
                  className: "an-subtitle",
                },
                "Entradas, economia e as m\xE9tricas do m\xEAs, o ano inteiro de uma vez.",
              ),
            ),
            /*#__PURE__*/ React.createElement(MonthNav, {
              label: String(year),
              onPrev: () => setYear((y) => y - 1),
              onNext: () => setYear((y) => y + 1),
              onToday: () => setYear(thisYear),
              atToday: year === thisYear,
              prevLabel: "Ano anterior",
              nextLabel: "Pr\xF3ximo ano",
            }),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "an-card",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "an-card__body",
              },
              hasYearData &&
                (hasEconomia
                  ? /*#__PURE__*/ React.createElement(EconomizadoSparkline, {
                      months: months,
                    })
                  : /*#__PURE__*/ React.createElement(
                      "p",
                      {
                        className: "an-no-economia",
                      },
                      "Sem Economia registrada em ",
                      year,
                      " \u2014 importe a aba",
                      " ",
                      /*#__PURE__*/ React.createElement("strong", null, "Economia"),
                      " em Configura\xE7\xF5es \u203A Google Sheets para ver a tend\xEAncia de quanto voc\xEA guardou (meta 20\u201330%).",
                    )),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "an-scroll",
                },
                /*#__PURE__*/ React.createElement(
                  "table",
                  {
                    className: "an-table",
                  },
                  /*#__PURE__*/ React.createElement(
                    "thead",
                    null,
                    /*#__PURE__*/ React.createElement(
                      "tr",
                      {
                        className: "an-thead-row",
                      },
                      /*#__PURE__*/ React.createElement(
                        "th",
                        {
                          scope: "col",
                          className: "col-mes",
                        },
                        "M\xEAs",
                      ),
                      /*#__PURE__*/ React.createElement(
                        "th",
                        {
                          scope: "col",
                        },
                        "Entradas",
                      ),
                      /*#__PURE__*/ React.createElement(
                        "th",
                        {
                          scope: "col",
                        },
                        "Economia",
                      ),
                      /*#__PURE__*/ React.createElement(
                        "th",
                        {
                          scope: "col",
                        },
                        /*#__PURE__*/ React.createElement(
                          InfoPopover,
                          {
                            term: "economizado",
                          },
                          "Economizado",
                        ),
                      ),
                      /*#__PURE__*/ React.createElement(
                        "th",
                        {
                          scope: "col",
                        },
                        "Performance",
                      ),
                      /*#__PURE__*/ React.createElement(
                        "th",
                        {
                          scope: "col",
                        },
                        "Custo de vida",
                      ),
                      /*#__PURE__*/ React.createElement(
                        "th",
                        {
                          scope: "col",
                        },
                        "Di\xE1rio m\xE9dio",
                      ),
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "tbody",
                    null,
                    months.map((m) => {
                      const empty =
                        m.income_cents === 0 && m.cost_of_living_cents === 0;
                      const pct = m.savings_rate_bps / 100;
                      return /*#__PURE__*/ React.createElement(
                        "tr",
                        {
                          key: m.month,
                          className: `an-tbody-row${empty ? " is-empty" : ""}`,
                        },
                        /*#__PURE__*/ React.createElement(
                          "td",
                          {
                            className: "col-mes",
                          },
                          MONTHS_PT[m.month - 1],
                        ),
                        /*#__PURE__*/ React.createElement(
                          "td",
                          null,
                          /*#__PURE__*/ React.createElement(Money, {
                            cents: m.income_cents,
                            size: "sm",
                            sign: "auto",
                          }),
                        ),
                        /*#__PURE__*/ React.createElement(
                          "td",
                          null,
                          /*#__PURE__*/ React.createElement(Money, {
                            cents: m.economia_cents,
                            size: "sm",
                          }),
                        ),
                        /*#__PURE__*/ React.createElement(
                          "td",
                          {
                            className: "an-pct",
                            style: {
                              color: empty ? "var(--text-faint)" : "var(--text)",
                            },
                          },
                          empty ? "—" : `${pct.toFixed(0)}%`,
                        ),
                        /*#__PURE__*/ React.createElement(
                          "td",
                          null,
                          /*#__PURE__*/ React.createElement(Money, {
                            cents: m.performance_cents,
                            size: "sm",
                            sign: "auto",
                          }),
                        ),
                        /*#__PURE__*/ React.createElement(
                          "td",
                          null,
                          /*#__PURE__*/ React.createElement(Money, {
                            cents: m.cost_of_living_cents,
                            size: "sm",
                          }),
                        ),
                        /*#__PURE__*/ React.createElement(
                          "td",
                          null,
                          empty
                            ? /*#__PURE__*/ React.createElement(
                                "span",
                                {
                                  style: {
                                    color: "var(--text-faint)",
                                  },
                                },
                                "\u2014",
                              )
                            : /*#__PURE__*/ React.createElement(Money, {
                                cents: m.real_daily_avg_cents,
                                size: "sm",
                              }),
                        ),
                      );
                    }),
                  ),
                  hasYearData &&
                    /*#__PURE__*/ React.createElement(
                      "tfoot",
                      null,
                      /*#__PURE__*/ React.createElement(
                        "tr",
                        {
                          className: "an-tfoot-row",
                        },
                        /*#__PURE__*/ React.createElement(
                          "th",
                          {
                            scope: "row",
                          },
                          "Total",
                        ),
                        /*#__PURE__*/ React.createElement(
                          "td",
                          null,
                          /*#__PURE__*/ React.createElement(Money, {
                            cents: totals.income,
                            size: "sm",
                            sign: "auto",
                          }),
                        ),
                        /*#__PURE__*/ React.createElement(
                          "td",
                          null,
                          /*#__PURE__*/ React.createElement(Money, {
                            cents: totals.economia,
                            size: "sm",
                          }),
                        ),
                        /*#__PURE__*/ React.createElement(
                          "td",
                          {
                            className: "an-pct",
                            title:
                              "Economizado no ano = total economizado \xF7 total de entradas (meta 20\u201330%)",
                            style: {
                              color: savingsColor,
                            },
                          },
                          annualSavingsPct,
                          "%",
                        ),
                        /*#__PURE__*/ React.createElement(
                          "td",
                          null,
                          /*#__PURE__*/ React.createElement(Money, {
                            cents: totals.performance,
                            size: "sm",
                            sign: "auto",
                          }),
                        ),
                        /*#__PURE__*/ React.createElement(
                          "td",
                          null,
                          /*#__PURE__*/ React.createElement(Money, {
                            cents: totals.cost,
                            size: "sm",
                          }),
                        ),
                        /*#__PURE__*/ React.createElement(
                          "td",
                          {
                            style: {
                              color: "var(--text-faint)",
                            },
                            title:
                              "Di\xE1rio m\xE9dio n\xE3o tem total anual \u2014 m\xE9dias n\xE3o se somam",
                          },
                          "\u2014",
                        ),
                      ),
                    ),
                ),
              ),
            ),
          ),
        );
      }
      window.AnnualScreen = AnnualScreen;
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "ui_kits/anuais/AnnualScreen.jsx",
      error: String((e && e.message) || e),
    });
  }

  // ui_kits/copilot/CopilotScreen.jsx
  try {
    (() => {
      /* Neko Finance — Tela Mia / Copiloto (stub Em desenvolvimento).
         Mostra o header da Mia com badge de aviso, texto explicativo e a seção
         "O que a Mia já sabe" com fatos determinísticos do método.
         Expõe window.CopilotScreen. */
      const NS = window.NekoFinanceDesignSystem_9bd1cd;
      const { Badge, MiaAvatar } = NS;
      const Icon = window.Icon;
      const copCSS = `
      .cop{display:flex;flex-direction:column;gap:var(--space-6);max-width:680px;}

      /* Painel principal da Mia */
      .cop-panel{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-lg);
        box-shadow:var(--shadow-1);padding:var(--space-7);}

      /* Cabeçalho: avatar + nome + badge */
      .cop-header{display:flex;align-items:center;gap:var(--space-5);margin-bottom:var(--space-5);}
      .cop-header__meta{flex:1;min-width:0;}
      .cop-header__label{font-size:var(--fs-micro);font-weight:var(--fw-bold);letter-spacing:var(--ls-caps);
        text-transform:uppercase;color:var(--text-faint);line-height:1;margin-bottom:3px;}
      .cop-header__name{font-size:var(--fs-h3);font-weight:var(--fw-bold);color:var(--text-strong);
        letter-spacing:var(--ls-snug);line-height:1.1;}

      /* Texto explicativo */
      .cop-desc{font-size:var(--fs-body);line-height:1.6;color:var(--text-muted);margin:0;}

      /* Seção "O que a Mia já sabe" */
      .cop-facts{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);
        box-shadow:var(--shadow-1);padding:var(--space-6);}
      .cop-facts__head{font-size:var(--fs-label);font-weight:var(--fw-semibold);letter-spacing:var(--ls-label);
        text-transform:uppercase;color:var(--text-muted);margin:0 0 var(--space-5);}
      .cop-facts__list{list-style:none;margin:0;padding:0;display:flex;flex-direction:column;gap:var(--space-4);}
      .cop-fact{display:flex;gap:var(--space-3);align-items:baseline;font-size:var(--fs-body);color:var(--text);
        line-height:1.5;}
      .cop-fact__arrow{color:var(--primary);flex:none;font-family:var(--font-mono);font-size:var(--fs-sm);}
      .cop-fact__money{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-weight:var(--fw-semibold);
        color:var(--text-strong);}

      /* Seção roadmap "O que a Mia vai fazer" */
      .cop-road{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);
        box-shadow:var(--shadow-1);padding:var(--space-6);}
      .cop-road__head{font-size:var(--fs-label);font-weight:var(--fw-semibold);letter-spacing:var(--ls-label);
        text-transform:uppercase;color:var(--text-muted);margin:0 0 var(--space-5);}
      .cop-road__list{list-style:none;margin:0;padding:0;display:flex;flex-direction:column;gap:var(--space-5);
        counter-reset:road;}
      .cop-roaditem{display:flex;gap:var(--space-4);font-size:var(--fs-body);color:var(--text);line-height:1.5;
        counter-increment:road;}
      .cop-roaditem__num{flex:none;width:22px;height:22px;border-radius:50%;background:var(--primary-quiet);
        color:var(--primary-quiet-text);font-size:var(--fs-micro);font-weight:var(--fw-bold);
        display:flex;align-items:center;justify-content:center;margin-top:1px;}
      `;
      function injectCopCSS() {
        if (document.getElementById("copilot-css")) return;
        const s = document.createElement("style");
        s.id = "copilot-css";
        s.textContent = copCSS;
        document.head.appendChild(s);
      }

      /* Fatos determinísticos representativos (valores fixos para o ui_kit) */
      const FACTS = [
        /*#__PURE__*/ React.createElement(
          React.Fragment,
          null,
          "Sua reserva cobre ",
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: "cop-fact__money",
            },
            "7,3",
          ),
          " meses de custo de vida (a meta m\xEDnima \xE9 6).",
        ),
        /*#__PURE__*/ React.createElement(
          React.Fragment,
          null,
          "No ano, voc\xEA economizou ",
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: "cop-fact__money",
            },
            "24%",
          ),
          " (refer\xEAncia 20\u201330%).",
        ),
        /*#__PURE__*/ React.createElement(
          React.Fragment,
          null,
          "Voc\xEA pode gastar at\xE9 ",
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: "cop-fact__money",
            },
            "R$ 312,40",
          ),
          " hoje sem furar suas metas.",
        ),
      ];
      const ROADMAP = [
        "Diagnóstico em linguagem natural: padrões de gasto, evolução da reserva e o peso real do crédito — sempre em modo leitura.",
        "Respostas a decisões: “posso comprar?”, “à vista ou parcelado?” — usando o saldo projetado, nunca cálculo improvisado.",
        "Escrita na planilha somente com a sua aprovação explícita, mostrando um diff antes → depois de cada alteração.",
      ];
      function CopilotScreen() {
        injectCopCSS();
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "cop",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "cop-panel",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "cop-header",
              },
              /*#__PURE__*/ React.createElement(MiaAvatar, {
                width: 48,
                height: 48,
              }),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "cop-header__meta",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "cop-header__label",
                  },
                  "Copiloto",
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "cop-header__name",
                  },
                  "Mia",
                ),
              ),
              /*#__PURE__*/ React.createElement(
                Badge,
                {
                  tone: "warning",
                },
                "Em desenvolvimento",
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "p",
              {
                className: "cop-desc",
              },
              "O chat da Mia ainda n\xE3o est\xE1 dispon\xEDvel nesta vers\xE3o. Tudo o que voc\xEA v\xEA no app hoje \xE9 calculado pelo motor determin\xEDstico \u2014 nada \xE9 gerado por IA.",
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "section",
            {
              "aria-labelledby": "cop-knows-title",
              className: "cop-facts",
            },
            /*#__PURE__*/ React.createElement(
              "h2",
              {
                id: "cop-knows-title",
                className: "cop-facts__head",
              },
              "O que a Mia j\xE1 sabe \xB7 n\xFAmeros do m\xE9todo, sem IA",
            ),
            /*#__PURE__*/ React.createElement(
              "ul",
              {
                className: "cop-facts__list",
              },
              FACTS.map((fact, i) =>
                /*#__PURE__*/ React.createElement(
                  "li",
                  {
                    key: i,
                    className: "cop-fact",
                  },
                  /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      className: "cop-fact__arrow",
                      "aria-hidden": "true",
                    },
                    "\u21B3",
                  ),
                  /*#__PURE__*/ React.createElement("span", null, fact),
                ),
              ),
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "section",
            {
              "aria-labelledby": "cop-road-title",
              className: "cop-road",
            },
            /*#__PURE__*/ React.createElement(
              "h2",
              {
                id: "cop-road-title",
                className: "cop-road__head",
              },
              "O que a Mia vai fazer",
            ),
            /*#__PURE__*/ React.createElement(
              "ol",
              {
                className: "cop-road__list",
              },
              ROADMAP.map((item, i) =>
                /*#__PURE__*/ React.createElement(
                  "li",
                  {
                    key: i,
                    className: "cop-roaditem",
                  },
                  /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      className: "cop-roaditem__num",
                      "aria-hidden": "true",
                    },
                    i + 1,
                  ),
                  /*#__PURE__*/ React.createElement("span", null, item),
                ),
              ),
            ),
          ),
        );
      }
      window.CopilotScreen = CopilotScreen;
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "ui_kits/copilot/CopilotScreen.jsx",
      error: String((e && e.message) || e),
    });
  }

  // ui_kits/dashboard/AppShell.jsx
  try {
    (() => {
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

        // Daily screens — mirrors src/shell/AppShell.tsx nav order (PT-BR).
        const NAV = [
          {
            key: "dashboard",
            label: "Dashboard",
            icon: "dashboard",
          },
          {
            key: "transactions",
            label: "Lançamentos",
            icon: "receipt",
          },
          {
            key: "totais",
            label: "Totais",
            icon: "calculator",
          },
          {
            key: "anuais",
            label: "Anual",
            icon: "trendingUp",
          },
          {
            key: "ano-inteiro",
            label: "Ano inteiro",
            icon: "layoutList",
          },
          {
            key: "economia-compare",
            label: "Economia comparada",
            icon: "gitCompare",
          },
          {
            key: "horizonte",
            label: "Horizonte",
            icon: "calendarRange",
          },
          {
            key: "tags",
            label: "Tags",
            icon: "tags",
          },
        ];
        // Secondary — settings, the demoted methodology ("Ajuda"), and Mia (a stub).
        const SYSTEM = [
          {
            key: "settings",
            label: "Configurações e privacidade",
            icon: "settings",
          },
          {
            key: "methodology",
            label: "Ajuda",
            icon: "help",
          },
          {
            key: "copilot",
            label: "Mia",
            icon: "sparkles",
          },
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
            React.createElement(Icon, {
              name: theme === "light" ? "moon" : "sun",
              size: 17,
            }),
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
            {
              className: "ak" + (dock ? " ak--dock" : ""),
            },
            // sidebar
            React.createElement(
              "aside",
              {
                className: "ak-side",
              },
              React.createElement(
                "div",
                {
                  className: "ak-brand",
                },
                React.createElement(Mark),
                React.createElement(
                  "span",
                  {
                    className: "ak-brand__name",
                  },
                  "Neko",
                ),
                React.createElement(
                  "span",
                  {
                    className: "ak-brand__tag",
                  },
                  React.createElement(Icon, {
                    name: "lock",
                    size: 11,
                  }),
                  "Local",
                ),
              ),
              React.createElement(
                "nav",
                {
                  className: "ak-nav",
                },
                React.createElement(
                  "div",
                  {
                    className: "ak-navh",
                  },
                  "Finanças",
                ),
                ...NAV.map((n) =>
                  React.createElement(
                    "button",
                    {
                      key: n.key,
                      className:
                        "ak-item" + (active === n.key ? " ak-item--active" : ""),
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
                          {
                            className: "ak-item__badge",
                          },
                          React.createElement(
                            Badge,
                            {
                              tone: "warning",
                              square: true,
                            },
                            n.badge,
                          ),
                        )
                      : null,
                  ),
                ),
                React.createElement(
                  "div",
                  {
                    className: "ak-navh",
                  },
                  "Sistema",
                ),
                ...SYSTEM.map((n) =>
                  React.createElement(
                    "button",
                    {
                      key: n.key,
                      className:
                        "ak-item" + (active === n.key ? " ak-item--active" : ""),
                      onClick: () => onNav(n.key),
                    },
                    React.createElement(Icon, {
                      name: n.icon,
                      size: 18,
                      className: "ak-item__ic",
                    }),
                    React.createElement("span", null, n.label),
                  ),
                ),
              ),
              React.createElement(
                "div",
                {
                  className: "ak-side__foot",
                },
                React.createElement(
                  "div",
                  {
                    className: "ak-conn",
                  },
                  React.createElement(
                    "span",
                    {
                      className: "ak-conn__ic",
                    },
                    React.createElement(Icon, {
                      name: "table",
                      size: 15,
                    }),
                  ),
                  React.createElement(
                    "div",
                    null,
                    React.createElement(
                      "div",
                      {
                        className: "ak-conn__t",
                      },
                      "Planilha conectada",
                    ),
                    React.createElement(
                      "div",
                      {
                        className: "ak-conn__s",
                      },
                      "Sincronizada há 2 min · somente leitura",
                    ),
                  ),
                ),
              ),
            ),
            // main
            React.createElement(
              "div",
              {
                className: "ak-main",
              },
              React.createElement(
                "header",
                {
                  className: "ak-top",
                },
                React.createElement(
                  "div",
                  {
                    className: "ak-top__titles",
                  },
                  React.createElement(
                    "div",
                    {
                      className: "ak-top__title",
                    },
                    title,
                  ),
                  crumb
                    ? React.createElement(
                        "div",
                        {
                          className: "ak-top__crumb",
                        },
                        crumb,
                      )
                    : null,
                ),
                React.createElement("div", {
                  className: "ak-spacer",
                }),
                React.createElement(
                  "label",
                  {
                    className: "ak-search",
                  },
                  React.createElement(Icon, {
                    name: "search",
                    size: 15,
                  }),
                  React.createElement("input", {
                    placeholder: "Buscar lançamentos…",
                  }),
                ),
                right || null,
                React.createElement(ThemeToggle),
                React.createElement(
                  "button",
                  {
                    className: "ak-iconbtn",
                  },
                  React.createElement(Icon, {
                    name: "bell",
                    size: 17,
                  }),
                ),
              ),
              React.createElement(
                "div",
                {
                  className: "ak-body" + (flush ? " ak-body--flush" : ""),
                },
                children,
              ),
            ),
            // dock
            dock
              ? React.createElement(
                  "aside",
                  {
                    className: "ak-dock",
                  },
                  dock,
                )
              : null,
          );
        }
        window.AppShell = AppShell;
      })();
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "ui_kits/dashboard/AppShell.jsx",
      error: String((e && e.message) || e),
    });
  }

  // ui_kits/dashboard/DashboardScreen.jsx
  try {
    (() => {
      /* Neko Finance — Dashboard screen (reconciled).
         "Quanto posso gastar hoje" — hero KPI + BalanceTrajectory + DailyCheckin + cards de análise.
         Todas as seções espelham os componentes reais: ColchaoCard, DailyCheckinCard, MonthLedgerCard,
         PerformanceCard, PrevisibilidadeCard, WriteBackPending, LastLoggedBanner.
         PT-BR copy · R$ em mono tabular · zero dependências externas.
         Expõe window.DashboardScreen. */

      const NS = window.NekoFinanceDesignSystem_9bd1cd;
      const {
        Badge,
        Button,
        Disclosure,
        Money,
        BalanceTrajectory,
        PhaseBadge,
        MovBadge,
        MonthNav,
      } = NS;
      const Icon = window.Icon;

      /* ---- CSS (once-only) ---- */
      (function injectDashCSS() {
        if (document.getElementById("dashboard-css")) return;
        const s = document.createElement("style");
        s.id = "dashboard-css";
        s.textContent = `
      /* Layout */
      .dash { display:flex; flex-direction:column; gap:var(--space-7); max-width:1100px; }

      /* Cards compartilhados */
      .dash-card {
        background: var(--surface);
        border: var(--bw-hair) solid var(--border);
        border-radius: var(--radius-md);
        box-shadow: var(--shadow-1);
      }
      .dash-card__head {
        display: flex; align-items: center; justify-content: space-between; gap: var(--space-5);
        padding: var(--space-5) var(--space-6) var(--space-4);
      }
      .dash-card__title {
        display: flex; align-items: center; gap: var(--space-3);
        font-size: var(--fs-sm); font-weight: var(--fw-semibold); color: var(--text-strong);
      }
      .dash-card__ic { color: var(--text-faint); }
      .dash-card__body { padding: var(--space-4) var(--space-6) var(--space-6); }

      /* Hero */
      .dash-hero {
        display: grid;
        grid-template-columns: 1fr 340px;
        gap: var(--space-7);
        padding: var(--space-7) var(--space-8);
        background: var(--surface);
        border: var(--bw-hair) solid var(--border);
        border-radius: var(--radius-lg);
        box-shadow: var(--shadow-2);
      }
      @media (max-width: 900px) { .dash-hero { grid-template-columns: 1fr; } }

      .dash-hero__lead { display:flex; flex-direction:column; gap: var(--space-4); min-width:0; }
      .dash-hero__label {
        font-size: var(--fs-sm); font-weight: var(--fw-medium);
        color: var(--text-muted); letter-spacing: var(--ls-label); text-transform: uppercase;
      }
      .dash-hero__kpi {
        font-family: var(--font-money); font-variant-numeric: tabular-nums;
        font-size: var(--fs-display-hero); font-weight: var(--fw-bold);
        color: var(--text-strong); letter-spacing: var(--ls-tight);
        line-height: var(--lh-tight);
        display: flex; align-items: baseline; gap: var(--space-3);
      }
      .dash-hero__kpi-suffix {
        font-family: var(--font-sans); font-size: var(--fs-body);
        font-weight: var(--fw-regular); color: var(--text-muted);
      }
      .dash-hero__reason {
        font-size: var(--fs-sm); color: var(--text-muted); max-width: 480px;
        line-height: var(--lh-normal);
      }
      .dash-hero__row { display:flex; align-items:center; gap: var(--space-6); flex-wrap: wrap; }
      .dash-hero__stats { display:flex; gap: var(--space-7); margin:0; padding:0; }
      .dash-hero__stats > div { display:flex; flex-direction:column; gap: var(--space-1); }
      .dash-hero__stats dt {
        font-size: var(--fs-micro); font-weight: var(--fw-medium); color: var(--text-faint);
        letter-spacing: var(--ls-label); text-transform: uppercase;
      }
      .dash-hero__stats dd {
        font-size: var(--fs-sm); font-weight: var(--fw-semibold); color: var(--text);
        font-family: var(--font-money); font-variant-numeric: tabular-nums;
        margin: 0;
      }

      /* Forecast aside */
      .dash-hero__forecast {
        display: flex; flex-direction: column; gap: var(--space-3);
        min-width: 0;
      }
      .dash-hero__forecast-head {
        display: flex; align-items: baseline; justify-content: space-between;
        gap: var(--space-3);
        font-size: var(--fs-sm); color: var(--text-muted);
      }
      .dash-hero__forecast-foot {
        font-size: var(--fs-micro); color: var(--text-faint); margin: 0;
        line-height: var(--lh-normal);
      }
      .dash-hero__forecast-foot .negative { color: var(--money-neg); }

      /* Déficit banner */
      .dash-deficit {
        display: flex; align-items: center; gap: var(--space-3);
        padding: var(--space-3) var(--space-5);
        background: var(--danger-tint);
        border: var(--bw-hair) solid var(--danger-500);
        border-radius: var(--radius-sm);
        font-size: var(--fs-sm); color: var(--money-neg);
      }

      /* Aviso de último lançamento */
      .dash-banner {
        display: flex; align-items: center; gap: var(--space-3);
        padding: var(--space-3) var(--space-5);
        background: var(--bg-subtle);
        border-radius: var(--radius-sm);
        font-size: var(--fs-sm); color: var(--text-muted);
      }
      .dash-banner__ic { color: var(--primary); flex-shrink: 0; }

      /* WriteBack pending */
      .dash-wb {
        display: grid; gap: var(--space-3);
        padding: var(--space-4) var(--space-5);
        background: var(--bg-subtle);
        border: var(--bw-hair) solid var(--border);
        border-radius: var(--radius-sm);
      }
      .dash-wb__head {
        display: flex; align-items: center; gap: var(--space-3);
        color: var(--warning-400); font-size: var(--fs-sm);
      }
      .dash-wb__actions { display:flex; gap: var(--space-3); flex-wrap: wrap; }

      /* Check-in diário */
      .dash-checkin__body { padding: var(--space-4) var(--space-6) var(--space-5); display:flex; flex-direction:column; gap:var(--space-4); }
      .dash-checkin__top {
        display: flex; align-items: baseline; justify-content: space-between;
        font-size: var(--fs-sm);
      }
      .dash-checkin__spent { font-family: var(--font-money); font-variant-numeric: tabular-nums; }
      .dash-checkin__bar-track {
        height: 6px; border-radius: var(--radius-pill); background: var(--bg-subtle); overflow: hidden;
      }
      .dash-checkin__bar-fill {
        height: 100%; border-radius: var(--radius-pill);
        background: var(--type-diario);
        transform-origin: left;
        transition: transform var(--dur-slow) var(--ease-entrance);
      }
      .dash-checkin__bar-fill--over { background: var(--danger-500); }
      @media (prefers-reduced-motion: reduce) {
        .dash-checkin__bar-fill { transition: none; }
      }
      .dash-checkin__kinds {
        display: flex; gap: var(--space-2); flex-wrap: wrap;
      }
      .dash-checkin__kind-btn {
        display: inline-flex; align-items: center; gap: var(--space-2);
        height: 32px; padding: 0 var(--space-3);
        border-radius: var(--radius-sm); cursor: pointer;
        border: var(--bw-hair) solid var(--border);
        background: transparent; color: var(--text);
        font-family: var(--font-sans); font-size: var(--fs-sm);
        transition: var(--t-hover);
      }
      .dash-checkin__kind-btn--active {
        background: var(--surface-selected); border-color: var(--primary);
      }
      .dash-checkin__inputs {
        display: flex; gap: var(--space-3); align-items: center;
      }
      .dash-checkin__input {
        flex: 1; height: 36px; padding: 0 var(--space-3);
        background: var(--bg-subtle);
        border: var(--bw-hair) solid var(--border-input);
        border-radius: var(--radius-xs);
        color: var(--text); font-family: var(--font-money); font-size: var(--fs-body);
      }
      .dash-checkin__desc {
        font-family: var(--font-sans);
      }
      .dash-checkin__hint {
        font-size: var(--fs-micro); color: var(--text-faint); margin: 0;
      }
      .dash-checkin__avg {
        font-size: var(--fs-micro); color: var(--text-faint); margin: 0;
      }

      /* Previsibilidade */
      .dash-predict__head-trusted {
        font-size: var(--fs-micro); color: var(--text-faint);
      }
      .dash-predict__ok { font-size: var(--fs-sm); color: var(--success-500); margin: 0; }
      .dash-predict__warn { font-size: var(--fs-sm); color: var(--money-neg); margin: 0; }
      .dash-predict__neutral { font-size: var(--fs-sm); color: var(--text-muted); margin: 0; }
      .dash-predict__rows { display:flex; flex-direction:column; gap: var(--space-4); margin-top: var(--space-4); }
      .dash-predict__row { display:flex; align-items:center; gap: var(--space-4); font-size: var(--fs-sm); }
      .dash-predict__month { width: 64px; color: var(--text-muted); flex-shrink:0; }
      .dash-predict__bar {
        flex: 1; height: 5px; border-radius: var(--radius-pill);
        background: var(--bg-subtle); overflow:hidden;
      }
      .dash-predict__fill { height:100%; background: var(--chart-1); border-radius: var(--radius-pill); }
      .dash-predict__pct { font-size: var(--fs-micro); color: var(--text-faint); white-space: nowrap; }
      .dash-predict__savings {
        margin-top: var(--space-5); padding-top: var(--space-4);
        border-top: var(--bw-hair) solid var(--border);
        font-size: var(--fs-sm); color: var(--text-muted);
      }

      /* Colchão */
      .dash-colchao__nums { display:flex; gap: var(--space-7); flex-wrap: wrap; margin-bottom: var(--space-5); }
      .dash-colchao__num { display:flex; flex-direction:column; gap: var(--space-1); }
      .dash-colchao__label { font-size: var(--fs-micro); color: var(--text-faint); letter-spacing: var(--ls-label); text-transform: uppercase; }
      .dash-colchao__val { font-family: var(--font-money); font-variant-numeric: tabular-nums; font-size: var(--fs-money-md); color: var(--text); }
      .dash-colchao__val--muted { color: var(--text-faint); }
      .dash-colchao__text { font-size: var(--fs-sm); color: var(--text-muted); margin: 0 0 var(--space-4); line-height: var(--lh-normal); }

      /* Performance por mês */
      .dash-perf__hint { font-size: var(--fs-micro); color: var(--text-faint); }
      .dash-perf__row {
        display: flex; gap: var(--space-5);
        padding: var(--space-4) var(--space-6) var(--space-6);
        flex-wrap: wrap;
      }
      .dash-perf__cell {
        flex: 1; min-width: 120px;
        display: flex; flex-direction:column; gap: var(--space-1);
        padding: var(--space-4) var(--space-5);
        background: var(--bg-subtle);
        border-radius: var(--radius-sm);
        border: var(--bw-hair) solid var(--border);
      }
      .dash-perf__cell.is-incomplete { opacity: 0.7; }
      .dash-perf__month { font-size: var(--fs-micro); color: var(--text-faint); letter-spacing: var(--ls-label); text-transform: uppercase; }
      .dash-perf__val { font-family: var(--font-money); font-variant-numeric: tabular-nums; font-size: var(--fs-money-md); color: var(--text); }
      .dash-perf__val--muted { color: var(--text-muted); }
      .dash-perf__rate { font-size: var(--fs-micro); color: var(--text-faint); }

      /* Dia a dia (grade do mês) */
      .dash-ledger-scroll { overflow-x: auto; -webkit-overflow-scrolling: touch; }
      .dash-ledger-table {
        width: 100%; border-collapse: collapse;
        font-size: var(--fs-sm); line-height: var(--lh-snug);
      }
      .dash-ledger-table thead th {
        padding: var(--space-3) var(--space-5);
        border-bottom: var(--bw-hair) solid var(--border);
        font-size: var(--fs-micro); font-weight: var(--fw-semibold);
        color: var(--text-faint); letter-spacing: var(--ls-label); text-transform: uppercase;
        text-align: right; white-space: nowrap;
      }
      .dash-ledger-table thead th:first-child { text-align: left; }
      .dash-ledger-table tbody td {
        padding: var(--space-3) var(--space-5);
        border-bottom: var(--bw-hair) solid var(--border);
        color: var(--text); font-family: var(--font-money); font-variant-numeric: tabular-nums;
        text-align: right; white-space: nowrap;
      }
      .dash-ledger-table tbody td:first-child { font-family: var(--font-sans); text-align: left; }
      .dash-ledger-table tbody tr.is-today td { background: var(--surface-selected); }
      .dash-ledger-table tfoot td, .dash-ledger-table tfoot th {
        padding: var(--space-3) var(--space-5);
        font-size: var(--fs-sm); font-weight: var(--fw-semibold);
        border-top: var(--bw-hair) solid var(--border-strong);
        font-family: var(--font-money); font-variant-numeric: tabular-nums;
        text-align: right; white-space: nowrap; color: var(--text);
      }
      .dash-ledger-table tfoot th { font-family: var(--font-sans); text-align: left; }
      .dash-today-tag {
        margin-left: var(--space-2);
        padding: 1px 5px;
        background: var(--surface-selected);
        border-radius: var(--radius-xs);
        font-size: var(--fs-micro); color: var(--primary);
        vertical-align: middle;
      }
      .money-pos { color: var(--money-pos); }
      .money-neg { color: var(--money-neg); }
      `;
        document.head.appendChild(s);
      })();

      /* ---- helpers ---- */
      function fmtBRL(cents) {
        const abs = Math.abs(cents);
        const n = (abs / 100).toLocaleString("pt-BR", {
          minimumFractionDigits: 2,
          maximumFractionDigits: 2,
        });
        return "R$ " + n;
      }
      function moneyColor(cents) {
        if (cents > 0) return "var(--money-pos)";
        if (cents < 0) return "var(--money-neg)";
        return "var(--text-muted)";
      }

      /* ---- Sub-componentes estáticos de demo ---- */

      /** Herói: "Pode gastar até" + BalanceTrajectory. */
      function HeroSection() {
        // Dados fictícios representativos para a tela de demo.
        const safeToSpend = 32700; // R$ 327,00
        const monthEndBalance = 215400; // R$ 2.154,00
        const today = "2026-06-21";
        const savingsBinds = false;
        const reserveMonths = 7.4;
        const txnCount = 226;

        // Trajetória diária fictícia — 30 dias de junho com tendência realista.
        const daily = Array.from(
          {
            length: 30,
          },
          (_, i) => {
            const d = String(i + 1).padStart(2, "0");
            const past = i < 21;
            const balance = past
              ? Math.round(3200 * 100 - i * 52 * 100 + (i % 7 === 0 ? 650000 : 0))
              : Math.round(2500 * 100 - (i - 20) * 48 * 100);
            return {
              date: `2026-06-${d}`,
              balance_cents: balance,
              projected: !past,
            };
          },
        );
        return /*#__PURE__*/ React.createElement(
          "section",
          {
            className: "dash-hero",
            "aria-label": "Quanto posso gastar hoje",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-hero__lead",
            },
            /*#__PURE__*/ React.createElement(
              "p",
              {
                className: "dash-hero__label",
              },
              "Pode gastar at\xE9",
            ),
            /*#__PURE__*/ React.createElement(
              "p",
              {
                className: "dash-hero__kpi",
              },
              fmtBRL(safeToSpend),
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "dash-hero__kpi-suffix",
                },
                "hoje",
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "p",
              {
                className: "dash-hero__reason",
              },
              savingsBinds
                ? "O menor de dois limites: respeita sua meta de guardar 25% no ano."
                : "O menor de dois limites: o que o caixa aguenta sem nenhum dia no vermelho.",
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "dash-hero__row",
              },
              /*#__PURE__*/ React.createElement(
                "dl",
                {
                  className: "dash-hero__stats",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  null,
                  /*#__PURE__*/ React.createElement("dt", null, "Reserva"),
                  /*#__PURE__*/ React.createElement(
                    "dd",
                    null,
                    reserveMonths.toFixed(1),
                    " meses",
                  ),
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  null,
                  /*#__PURE__*/ React.createElement("dt", null, "Lan\xE7amentos"),
                  /*#__PURE__*/ React.createElement("dd", null, txnCount),
                ),
              ),
              /*#__PURE__*/ React.createElement(
                Button,
                {
                  variant: "secondary",
                  size: "sm",
                  iconLeft: /*#__PURE__*/ React.createElement(Icon, {
                    name: "sparkles",
                    size: 15,
                  }),
                },
                "Conhecer a Mia",
              ),
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "aside",
            {
              className: "dash-hero__forecast",
              "aria-label": "Saldo projetado do m\xEAs",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "dash-hero__forecast-head",
              },
              /*#__PURE__*/ React.createElement("span", null, "Saldo no fim de junho"),
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  style: {
                    fontFamily: "var(--font-money)",
                    fontVariantNumeric: "tabular-nums",
                    fontSize: "var(--fs-sm)",
                    fontWeight: "var(--fw-semibold)",
                    color: moneyColor(monthEndBalance),
                  },
                },
                fmtBRL(monthEndBalance),
              ),
            ),
            /*#__PURE__*/ React.createElement(BalanceTrajectory, {
              daily: daily,
              today: today,
              variant: "compact",
            }),
            /*#__PURE__*/ React.createElement(
              "p",
              {
                className: "dash-hero__forecast-foot",
              },
              "Como seu saldo deve evoluir at\xE9 o fim do m\xEAs.",
            ),
          ),
        );
      }

      /** Aviso: último lançamento foi há 2 dias (demo). */
      function LastLoggedBanner() {
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "dash-banner",
            role: "status",
          },
          /*#__PURE__*/ React.createElement(Icon, {
            name: "calendar",
            size: 15,
            style: {
              color: "var(--primary)",
              flexShrink: 0,
            },
          }),
          /*#__PURE__*/ React.createElement(
            "span",
            null,
            "Voc\xEA lan\xE7ou pela \xFAltima vez h\xE1 2 dias.",
          ),
        );
      }

      /** Check-in diário — versão estática do DailyCheckinCard. */
      function DailyCheckinCard() {
        const [kind, setKind] = React.useState("diario");
        const ceiling = 32700;
        const spent = 14500;
        const remaining = ceiling - spent;
        const pct = Math.min(100, Math.round((spent / ceiling) * 100));
        const overspent = remaining < 0;
        const KINDS = ["entrada", "saida", "diario", "cartao", "economia"];
        return /*#__PURE__*/ React.createElement(
          "section",
          {
            "aria-labelledby": "dash-checkin-title",
            className: "dash-card",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-card__head",
            },
            /*#__PURE__*/ React.createElement(
              "span",
              {
                style: {
                  display: "flex",
                  flexDirection: "column",
                  gap: 2,
                },
              },
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "dash-card__title",
                  id: "dash-checkin-title",
                },
                /*#__PURE__*/ React.createElement(Icon, {
                  name: "calendar",
                  size: 16,
                  className: "dash-card__ic",
                  "aria-hidden": "true",
                }),
                "Di\xE1rio de hoje",
              ),
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  style: {
                    fontSize: "var(--fs-micro)",
                    color: "var(--text-faint)",
                  },
                },
                "Di\xE1rio, cart\xE3o ou sa\xEDda \u2014 registre o que aconteceu hoje",
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "span",
              {
                style: {
                  fontSize: "var(--fs-sm)",
                  fontWeight: "var(--fw-semibold)",
                  color: overspent ? "var(--danger-500)" : "var(--text-muted)",
                },
              },
              overspent
                ? `${fmtBRL(-remaining)} acima do teto`
                : `${fmtBRL(remaining)} disponível`,
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-checkin__body",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "dash-checkin__top",
              },
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  style: {
                    color: "var(--text-muted)",
                  },
                },
                "Di\xE1rio registrado hoje",
              ),
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "dash-checkin__spent",
                },
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    style: {
                      fontFamily: "var(--font-money)",
                      fontVariantNumeric: "tabular-nums",
                      fontWeight: "var(--fw-bold)",
                    },
                  },
                  fmtBRL(spent),
                ),
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    style: {
                      color: "var(--text-faint)",
                      fontWeight: "var(--fw-regular)",
                    },
                  },
                  " / ",
                  fmtBRL(ceiling),
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement("progress", {
              value: pct,
              max: 100,
              "aria-label": `${pct}% do teto diário usado`,
              style: {
                position: "absolute",
                width: 1,
                height: 1,
                overflow: "hidden",
                clip: "rect(0,0,0,0)",
              },
            }),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                "aria-hidden": "true",
                className: "dash-checkin__bar-track",
              },
              /*#__PURE__*/ React.createElement("div", {
                className: `dash-checkin__bar-fill${overspent ? " dash-checkin__bar-fill--over" : ""}`,
                style: {
                  width: "100%",
                  transform: `scaleX(${pct / 100})`,
                },
              }),
            ),
            /*#__PURE__*/ React.createElement(
              "p",
              {
                className: "dash-checkin__avg",
              },
              "M\xE9dia do m\xEAs: R$\xA0145,00/dia",
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "dash-checkin__kinds",
                role: "radiogroup",
                "aria-label": "Tipo de movimento",
              },
              KINDS.map((k) =>
                /*#__PURE__*/ React.createElement(
                  "button",
                  {
                    key: k,
                    type: "button",
                    role: "radio",
                    "aria-checked": kind === k,
                    disabled: k === "economia",
                    onClick: () => setKind(k),
                    className: `dash-checkin__kind-btn${kind === k ? " dash-checkin__kind-btn--active" : ""}`,
                    style:
                      k === "economia"
                        ? {
                            opacity: 0.5,
                            cursor: "not-allowed",
                          }
                        : {},
                  },
                  /*#__PURE__*/ React.createElement(MovBadge, {
                    kind: k,
                    showLabel: true,
                    size: 14,
                  }),
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement("input", {
              "aria-label": "Descri\xE7\xE3o (opcional)",
              placeholder:
                "Descri\xE7\xE3o (opcional) \u2014 ex.: mercado, aluguel\u2026",
              className: "dash-checkin__input dash-checkin__desc",
              defaultValue: "",
            }),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "dash-checkin__inputs",
              },
              /*#__PURE__*/ React.createElement("input", {
                "aria-label": "Valor do lan\xE7amento (R$)",
                inputMode: "decimal",
                placeholder: "Valor de hoje (R$)",
                className: "dash-checkin__input",
              }),
              /*#__PURE__*/ React.createElement(
                Button,
                {
                  variant: "primary",
                },
                "Registrar",
              ),
            ),
            kind === "saida" &&
              /*#__PURE__*/ React.createElement(
                "p",
                {
                  className: "dash-checkin__hint",
                },
                "Sa\xEDda = despesa fixa do m\xEAs \u2014 contas, fatura no vencimento.",
              ),
            kind === "cartao" &&
              /*#__PURE__*/ React.createElement(
                "p",
                {
                  className: "dash-checkin__hint",
                },
                "Cart\xE3o = compra no cr\xE9dito (entra na fatura).",
              ),
            kind === "entrada" &&
              /*#__PURE__*/ React.createElement(
                "p",
                {
                  className: "dash-checkin__hint",
                },
                "Entrada = renda recebida no m\xEAs.",
              ),
          ),
        );
      }

      /** PrevisibilidadeCard — versão estática. */
      function PrevisibilidadeCard() {
        const incompleteMonths = [
          {
            label: "julho",
            pct: 38,
            falta: 182400,
          },
          {
            label: "agosto",
            pct: 12,
            falta: 274100,
          },
        ];
        return /*#__PURE__*/ React.createElement(
          "section",
          {
            "aria-labelledby": "dash-predict-title",
            className: "dash-card",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-card__head",
            },
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "dash-card__title",
                id: "dash-predict-title",
              },
              /*#__PURE__*/ React.createElement(Icon, {
                name: "calendarRange",
                size: 16,
                className: "dash-card__ic",
                "aria-hidden": "true",
              }),
              "Previsibilidade",
            ),
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "dash-predict__head-trusted",
              },
              "confi\xE1vel at\xE9 ",
              /*#__PURE__*/ React.createElement("strong", null, "junho"),
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-card__body",
            },
            /*#__PURE__*/ React.createElement(
              "p",
              {
                className: "dash-predict__warn",
              },
              "A partir de ",
              /*#__PURE__*/ React.createElement("strong", null, "julho"),
              " faltam",
              " ",
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  style: {
                    fontFamily: "var(--font-money)",
                    fontVariantNumeric: "tabular-nums",
                  },
                },
                "R$\xA04.564,00",
              ),
              " ",
              "de gastos n\xE3o lan\xE7ados. A proje\xE7\xE3o est\xE1 otimista at\xE9 voc\xEA pr\xE9-lan\xE7ar.",
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "dash-predict__rows",
              },
              incompleteMonths.map((m) =>
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    key: m.label,
                    className: "dash-predict__row",
                    "aria-label": `${m.label}: ${m.pct}% do gasto típico lançado, falta ${fmtBRL(m.falta)}`,
                  },
                  /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      className: "dash-predict__month",
                    },
                    m.label,
                  ),
                  /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      className: "dash-predict__bar",
                      "aria-hidden": "true",
                    },
                    /*#__PURE__*/ React.createElement("span", {
                      className: "dash-predict__fill",
                      style: {
                        width: `${m.pct}%`,
                      },
                    }),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      className: "dash-predict__pct",
                    },
                    m.pct,
                    "% \xB7 falta ",
                    fmtBRL(m.falta),
                  ),
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement(
              Disclosure,
              {
                title: "Como pr\xE9-lan\xE7ar o ano",
              },
              /*#__PURE__*/ React.createElement(
                "p",
                {
                  style: {
                    fontSize: "var(--fs-sm)",
                    color: "var(--text-muted)",
                    margin: 0,
                    lineHeight: "var(--lh-normal)",
                  },
                },
                "Em cada m\xEAs \xE0 frente, lance o ",
                /*#__PURE__*/ React.createElement("strong", null, "saldo de hoje"),
                " (s\xF3 conta-corrente), o ",
                /*#__PURE__*/ React.createElement("strong", null, "sal\xE1rio"),
                " conservador, as",
                " ",
                /*#__PURE__*/ React.createElement("strong", null, "contas fixas"),
                ", a ",
                /*#__PURE__*/ React.createElement(
                  "strong",
                  null,
                  "fatura do cart\xE3o",
                ),
                " no vencimento e o ",
                /*#__PURE__*/ React.createElement("strong", null, "di\xE1rio estimado"),
                " em todos os dias. Futuro vazio engana a previs\xE3o.",
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "p",
              {
                className: "dash-predict__savings",
              },
              "Economizado no ano: ",
              /*#__PURE__*/ React.createElement("strong", null, "8%"),
              " realizado, refer\xEAncia 20 a 30%",
            ),
          ),
        );
      }

      /** ColchaoCard — versão estática. */
      function ColchaoCard() {
        const colchaoCents = 183200;
        const registeredEconomia = 0;
        const realizedRatePct = "8.4";
        return /*#__PURE__*/ React.createElement(
          "section",
          {
            "aria-labelledby": "dash-colchao-title",
            className: "dash-card",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-card__head",
            },
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "dash-card__title",
                id: "dash-colchao-title",
              },
              /*#__PURE__*/ React.createElement(Icon, {
                name: "sparkles",
                size: 16,
                className: "dash-card__ic",
                "aria-hidden": "true",
              }),
              "Seu colch\xE3o",
            ),
            /*#__PURE__*/ React.createElement(PhaseBadge, {
              phase: "calibrate",
            }),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-card__body",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "dash-colchao__nums",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "dash-colchao__num",
                },
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "dash-colchao__label",
                  },
                  "Economia registrada",
                ),
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: `dash-colchao__val${registeredEconomia <= 0 ? " dash-colchao__val--muted" : ""}`,
                    style: {
                      color:
                        registeredEconomia > 0
                          ? "var(--money-pos)"
                          : "var(--text-faint)",
                    },
                  },
                  fmtBRL(registeredEconomia),
                ),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "dash-colchao__num",
                },
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "dash-colchao__label",
                  },
                  "Colch\xE3o este ano (sobra at\xE9 hoje)",
                ),
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "dash-colchao__val",
                    style: {
                      color: "var(--money-pos)",
                    },
                  },
                  fmtBRL(colchaoCents),
                  " \xB7 ",
                  realizedRatePct,
                  "%",
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "p",
              {
                className: "dash-colchao__text",
              },
              "Voc\xEA guarda o que sobra como colch\xE3o para cobrir os meses negativos sem sacar investimento. Adapta\xE7\xE3o v\xE1lida do m\xE9todo.",
            ),
            /*#__PURE__*/ React.createElement(
              Disclosure,
              {
                title: "Pr\xF3ximo n\xEDvel, quando quiser",
              },
              /*#__PURE__*/ React.createElement(
                "p",
                {
                  style: {
                    fontSize: "var(--fs-sm)",
                    color: "var(--text-muted)",
                    margin: 0,
                    lineHeight: "var(--lh-normal)",
                  },
                },
                "Registrar a Economia (meta 20 a 30% da renda) como uma sa\xEDda mensal e separar a reserva. Isso vira h\xE1bito e protege de sacar investimento na hora errada.",
              ),
            ),
          ),
        );
      }

      /** PerformanceCard — versão estática. */
      function PerformanceCard() {
        const months = [
          {
            label: "junho",
            performance: 53200,
            rate: 8,
            incomplete: false,
          },
          {
            label: "julho",
            performance: 71400,
            rate: 0,
            incomplete: true,
          },
          {
            label: "agosto",
            performance: 60100,
            rate: 0,
            incomplete: true,
          },
          {
            label: "setembro",
            performance: 58900,
            rate: 0,
            incomplete: true,
          },
        ];
        return /*#__PURE__*/ React.createElement(
          "section",
          {
            "aria-labelledby": "dash-perf-title",
            className: "dash-card",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-card__head",
            },
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "dash-card__title",
                id: "dash-perf-title",
                title:
                  "Caixa n\xE3o \xE9 poupan\xE7a: um m\xEAs pode ter saldo positivo e ainda assim performance baixa.",
              },
              /*#__PURE__*/ React.createElement(Icon, {
                name: "trendingUp",
                size: 16,
                className: "dash-card__ic",
                "aria-hidden": "true",
              }),
              "Performance por m\xEAs",
            ),
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "dash-perf__hint",
              },
              "refer\xEAncia anual 20\u201330%",
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-perf__row",
            },
            months.map((m) =>
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  key: m.label,
                  className: `dash-perf__cell${m.incomplete ? " is-incomplete" : ""}`,
                  "aria-label": m.incomplete
                    ? `${m.label}: incompleto, projeção otimista`
                    : `${m.label}: performance ${fmtBRL(m.performance)}, economizado ${m.rate}%`,
                },
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "dash-perf__month",
                  },
                  m.label,
                ),
                m.incomplete
                  ? /*#__PURE__*/ React.createElement(
                      React.Fragment,
                      null,
                      /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          className: `dash-perf__val dash-perf__val--muted`,
                        },
                        fmtBRL(m.performance),
                      ),
                      /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          className: "dash-perf__rate",
                          style: {
                            color: "var(--warning-500)",
                          },
                        },
                        /*#__PURE__*/ React.createElement(Icon, {
                          name: "alertTriangle",
                          size: 11,
                          style: {
                            verticalAlign: "-1px",
                            marginRight: 3,
                          },
                        }),
                        "incompleto",
                      ),
                    )
                  : /*#__PURE__*/ React.createElement(
                      React.Fragment,
                      null,
                      /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          className: "dash-perf__val",
                          style: {
                            color: "var(--money-pos)",
                          },
                        },
                        fmtBRL(m.performance),
                      ),
                      /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          className: "dash-perf__rate",
                        },
                        "economizado ",
                        m.rate,
                        "%",
                      ),
                    ),
              ),
            ),
          ),
        );
      }

      /** MonthLedgerCard — versão estática "Dia a dia". */
      function MonthLedgerCard() {
        const [ym, setYm] = React.useState("2026-06");
        const today = "2026-06-21";
        const year = 2026;
        const monthLabel = "Junho";

        // Amostra representativa: alguns dias com dados
        const rows = [
          {
            date: "2026-06-18",
            label: "18/06",
            entrada: 0,
            saida: 0,
            diario: 4200,
            saldo: 324800,
          },
          {
            date: "2026-06-19",
            label: "19/06",
            entrada: 0,
            saida: 85000,
            diario: 0,
            saldo: 239800,
          },
          {
            date: "2026-06-20",
            label: "20/06",
            entrada: 0,
            saida: 0,
            diario: 6700,
            saldo: 233100,
          },
          {
            date: "2026-06-21",
            label: "21/06",
            entrada: 0,
            saida: 0,
            diario: 14500,
            saldo: 218600,
          },
          {
            date: "2026-06-22",
            label: "22/06",
            entrada: 0,
            saida: 0,
            diario: null,
            saldo: null,
          },
          {
            date: "2026-06-23",
            label: "23/06",
            entrada: 0,
            saida: 0,
            diario: null,
            saldo: null,
          },
        ];

        // Saldo heatmap simplificado
        function saldoStyle(cents) {
          if (cents == null) return {};
          if (cents < 0)
            return {
              background: "rgba(224, 98, 91, 0.32)",
              color: "var(--text)",
            };
          if (cents < 50000)
            return {
              background: "rgba(224, 163, 62, 0.16)",
              color: "var(--text)",
            };
          if (cents < 200000)
            return {
              background: "rgba(52, 185, 129, 0.15)",
              color: "var(--text)",
            };
          return {
            background: "rgba(52, 185, 129, 0.30)",
            color: "var(--text)",
          };
        }
        const foot = {
          entrada: rows.reduce((s, r) => s + (r.entrada || 0), 0),
          saida: rows.reduce((s, r) => s + (r.saida || 0), 0),
          diario: rows.reduce((s, r) => s + (r.diario || 0), 0),
        };
        foot.saidaTotal = foot.saida + foot.diario;
        foot.performance = foot.entrada - foot.saidaTotal;
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "dash-card",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-card__head",
            },
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "dash-card__title",
              },
              /*#__PURE__*/ React.createElement(Icon, {
                name: "calendarRange",
                size: 16,
                className: "dash-card__ic",
              }),
              "Dia a dia",
            ),
            /*#__PURE__*/ React.createElement(MonthNav, {
              label: `${monthLabel} de ${year}`,
              onPrev: () => {},
              onNext: () => {},
              onToday: () => {},
              atToday: true,
              prevLabel: "M\xEAs anterior",
              nextLabel: "Pr\xF3ximo m\xEAs",
            }),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-card__body",
              style: {
                padding: 0,
              },
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "dash-ledger-scroll",
              },
              /*#__PURE__*/ React.createElement(
                "table",
                {
                  className: "dash-ledger-table",
                },
                /*#__PURE__*/ React.createElement(
                  "thead",
                  null,
                  /*#__PURE__*/ React.createElement(
                    "tr",
                    null,
                    /*#__PURE__*/ React.createElement(
                      "th",
                      {
                        scope: "col",
                        style: {
                          textAlign: "left",
                        },
                      },
                      "Data",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "th",
                      {
                        scope: "col",
                      },
                      "Entrada",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "th",
                      {
                        scope: "col",
                        title: "Sa\xEDdas fixas e a fatura do cart\xE3o no vencimento",
                      },
                      "Sa\xEDda",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "th",
                      {
                        scope: "col",
                      },
                      "Di\xE1rio",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "th",
                      {
                        scope: "col",
                      },
                      "Saldo",
                    ),
                  ),
                ),
                /*#__PURE__*/ React.createElement(
                  "tbody",
                  null,
                  rows.map((r) =>
                    /*#__PURE__*/ React.createElement(
                      "tr",
                      {
                        key: r.date,
                        className: r.date === today ? "is-today" : "",
                      },
                      /*#__PURE__*/ React.createElement(
                        "td",
                        {
                          style: {
                            fontFamily: "var(--font-sans)",
                          },
                        },
                        r.label,
                        r.date === today &&
                          /*#__PURE__*/ React.createElement(
                            "span",
                            {
                              className: "dash-today-tag",
                            },
                            "hoje",
                          ),
                      ),
                      /*#__PURE__*/ React.createElement(
                        "td",
                        {
                          style: {
                            textAlign: "right",
                          },
                        },
                        r.entrada
                          ? /*#__PURE__*/ React.createElement(
                              "span",
                              {
                                className: "money-pos",
                              },
                              fmtBRL(r.entrada),
                            )
                          : "—",
                      ),
                      /*#__PURE__*/ React.createElement(
                        "td",
                        {
                          style: {
                            textAlign: "right",
                          },
                        },
                        r.saida ? fmtBRL(r.saida) : "—",
                      ),
                      /*#__PURE__*/ React.createElement(
                        "td",
                        {
                          style: {
                            textAlign: "right",
                          },
                        },
                        r.diario ? fmtBRL(r.diario) : "—",
                      ),
                      /*#__PURE__*/ React.createElement(
                        "td",
                        {
                          style: {
                            textAlign: "right",
                            ...saldoStyle(r.saldo),
                          },
                        },
                        r.saldo != null ? fmtBRL(r.saldo) : "—",
                      ),
                    ),
                  ),
                ),
                /*#__PURE__*/ React.createElement(
                  "tfoot",
                  null,
                  /*#__PURE__*/ React.createElement(
                    "tr",
                    null,
                    /*#__PURE__*/ React.createElement(
                      "th",
                      {
                        scope: "row",
                      },
                      "Total",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "td",
                      {
                        style: {
                          textAlign: "right",
                        },
                      },
                      foot.entrada > 0
                        ? /*#__PURE__*/ React.createElement(
                            "span",
                            {
                              className: "money-pos",
                            },
                            fmtBRL(foot.entrada),
                          )
                        : "—",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "td",
                      {
                        style: {
                          textAlign: "right",
                        },
                      },
                      fmtBRL(foot.saida),
                    ),
                    /*#__PURE__*/ React.createElement(
                      "td",
                      {
                        style: {
                          textAlign: "right",
                        },
                      },
                      fmtBRL(foot.diario),
                    ),
                    /*#__PURE__*/ React.createElement(
                      "td",
                      {
                        style: {
                          textAlign: "right",
                          color: "var(--text-faint)",
                        },
                      },
                      "\u2014",
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "tr",
                    null,
                    /*#__PURE__*/ React.createElement(
                      "th",
                      {
                        scope: "row",
                      },
                      "Sa\xEDda Total",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "td",
                      {
                        colSpan: 3,
                        style: {
                          textAlign: "right",
                          color: "var(--text-faint)",
                          fontSize: "var(--fs-micro)",
                        },
                      },
                      "sa\xEDdas + di\xE1rio",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "td",
                      {
                        style: {
                          textAlign: "right",
                        },
                      },
                      fmtBRL(foot.saidaTotal),
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "tr",
                    null,
                    /*#__PURE__*/ React.createElement(
                      "th",
                      {
                        scope: "row",
                        title:
                          "Resultado cont\xE1bil do m\xEAs: entradas menos sa\xEDda total.",
                      },
                      "Resultado do m\xEAs",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "td",
                      {
                        colSpan: 3,
                        style: {
                          textAlign: "right",
                          color: "var(--text-faint)",
                          fontSize: "var(--fs-micro)",
                        },
                      },
                      "entradas \u2212 sa\xEDda total",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "td",
                      {
                        style: {
                          textAlign: "right",
                          color: moneyColor(foot.performance),
                        },
                      },
                      foot.performance >= 0 ? "" : "−",
                      fmtBRL(Math.abs(foot.performance)),
                    ),
                  ),
                ),
              ),
            ),
          ),
        );
      }

      /** WriteBack pending — versão estática (1 célula local pendente de envio). */
      function WriteBackPending() {
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "dash-wb",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-wb__head",
            },
            /*#__PURE__*/ React.createElement(Icon, {
              name: "download",
              size: 15,
              style: {
                flexShrink: 0,
              },
              "aria-hidden": "true",
            }),
            /*#__PURE__*/ React.createElement(
              "span",
              {
                "aria-live": "polite",
              },
              "1 c\xE9lula local \u2192 planilha pendente",
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-wb__actions",
            },
            /*#__PURE__*/ React.createElement(
              Button,
              {
                variant: "primary",
                size: "sm",
              },
              "Sincronizar",
            ),
            /*#__PURE__*/ React.createElement(
              Button,
              {
                variant: "ghost",
                size: "sm",
              },
              "Revisar e enviar",
            ),
          ),
        );
      }

      /* ---- Tela completa ---- */
      function DashboardScreen(props) {
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "dash",
          },
          /*#__PURE__*/ React.createElement(HeroSection, null),
          /*#__PURE__*/ React.createElement(WriteBackPending, null),
          /*#__PURE__*/ React.createElement(LastLoggedBanner, null),
          /*#__PURE__*/ React.createElement(DailyCheckinCard, null),
          /*#__PURE__*/ React.createElement(PrevisibilidadeCard, null),
          /*#__PURE__*/ React.createElement(ColchaoCard, null),
          /*#__PURE__*/ React.createElement(PerformanceCard, null),
          /*#__PURE__*/ React.createElement(MonthLedgerCard, null),
        );
      }
      window.DashboardScreen = DashboardScreen;
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "ui_kits/dashboard/DashboardScreen.jsx",
      error: String((e && e.message) || e),
    });
  }

  // ui_kits/economia-compare/EconomiaCompareScreen.jsx
  try {
    (() => {
      /* Neko Finance — Economia comparada (ui_kit).
         Dois anos lado a lado: Entradas · Economia · Economizado% mês a mês.
         Espelha EconomiaCompareScreen.tsx — PT-BR · R$ mono tabular · zero dependências.
         Expõe window.EconomiaCompareScreen. */

      const NS = window.NekoFinanceDesignSystem_9bd1cd;
      const { Money, MonthNav, InfoPopover, EmptyState } = NS;
      const Icon = window.Icon;

      /* ---- CSS (once-only) ---- */
      (function injectEconomiaCompareCSS() {
        if (document.getElementById("economia-compare-css")) return;
        const s = document.createElement("style");
        s.id = "economia-compare-css";
        s.textContent = `
      /* Layout da tela */
      .ec { max-width: 860px; margin: 0 auto; padding: var(--space-2); }

      /* Cabeçalho */
      .ec-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-4);
        margin-bottom: var(--space-6);
        flex-wrap: wrap;
      }
      .ec-header__lead { min-width: 0; }
      .ec-title {
        font-size: var(--fs-h2);
        font-weight: var(--fw-bold);
        letter-spacing: var(--ls-tight);
        color: var(--text-strong);
        margin: 0;
        line-height: var(--lh-snug);
      }
      .ec-subtitle {
        color: var(--text-muted);
        font-size: var(--fs-sm);
        margin: var(--space-1) 0 0;
        line-height: var(--lh-normal);
      }

      /* Card */
      .dash-card {
        background: var(--surface);
        border: var(--bw-hair) solid var(--border);
        border-radius: var(--radius-md);
        box-shadow: var(--shadow-1);
      }
      .dash-card__body { padding: var(--space-4) var(--space-6) var(--space-6); }

      /* Tabela */
      .ec-scroll { overflow-x: auto; -webkit-overflow-scrolling: touch; }
      .ec-table {
        width: 100%;
        border-collapse: collapse;
        font-variant-numeric: tabular-nums;
      }

      /* Cabeçalhos */
      .ec-th {
        text-align: right;
        font-size: var(--fs-label);
        font-weight: var(--fw-semibold);
        letter-spacing: var(--ls-label);
        text-transform: uppercase;
        color: var(--text-muted);
        padding: var(--space-3) var(--space-4);
        white-space: nowrap;
      }
      .ec-th--left { text-align: left; }
      .ec-th--year {
        text-align: center;
        color: var(--text-strong);
        font-size: var(--fs-sm);
        font-weight: var(--fw-semibold);
        border-bottom: 2px solid var(--border-strong);
        letter-spacing: 0;
        text-transform: none;
        padding: var(--space-3) var(--space-4);
      }
      .ec-th--year-a {
        border-right: var(--bw-strong) solid var(--border-strong);
      }
      .ec-th--group-end {
        border-right: var(--bw-strong) solid var(--border-strong);
      }
      .ec-th--divider {
        border-left: var(--bw-strong) solid var(--border-strong);
      }

      /* Células numéricas */
      .ec-td {
        text-align: right;
        padding: var(--space-3) var(--space-4);
        font-variant-numeric: tabular-nums;
      }
      .ec-td--month {
        padding: var(--space-3) var(--space-4);
        font-weight: var(--fw-semibold);
        color: var(--text);
        text-align: left;
        white-space: nowrap;
      }
      .ec-td--divider {
        border-left: var(--bw-strong) solid var(--border-strong);
      }
      .ec-td--group-end {
        border-right: var(--bw-strong) solid var(--border-strong);
      }

      /* Linhas */
      .ec-row { border-bottom: var(--bw-hair) solid var(--border); }
      .ec-row:last-child { border-bottom: none; }
      .ec-row--head { border-bottom: var(--bw-hair) solid var(--border); }
      .ec-row--foot {
        border-top: var(--bw-strong) solid var(--border-strong);
        font-weight: var(--fw-bold);
      }

      /* Total label */
      .ec-td--total {
        padding: var(--space-3) var(--space-4);
        text-transform: uppercase;
        letter-spacing: var(--ls-label);
        font-size: var(--fs-label);
        color: var(--text);
        font-weight: var(--fw-bold);
        white-space: nowrap;
      }

      /* Taxa economizado% — colorida semanticamente */
      .ec-rate { font-family: var(--font-money); font-variant-numeric: tabular-nums; }
      .ec-rate--strong { color: var(--primary); }
      .ec-rate--ok { color: var(--success-400); }
      .ec-rate--warn { color: var(--warning-400); }
      .ec-rate--faint { color: var(--text-faint); }

      /* Legenda de referência */
      .ec-legend {
        display: flex;
        gap: var(--space-5);
        flex-wrap: wrap;
        padding: var(--space-4) var(--space-5);
        border-top: var(--bw-hair) solid var(--border);
        font-size: var(--fs-micro);
        color: var(--text-faint);
        align-items: center;
      }
      .ec-legend__dot {
        display: inline-block;
        width: 8px; height: 8px;
        border-radius: var(--radius-circle);
        margin-right: var(--space-2);
        flex-shrink: 0;
        vertical-align: middle;
      }
      .ec-legend__item { display: flex; align-items: center; gap: 0; white-space: nowrap; }

      @media (prefers-reduced-motion: reduce) {
        * { transition: none !important; animation: none !important; }
      }
      `;
        document.head.appendChild(s);
      })();

      /* ---- Dados de demo ---- */

      // SAVINGS_MIN_PCT = SAVINGS_MIN_BPS / 100 = 2000 / 100 = 20%
      const SAVINGS_MIN_PCT = 20;
      const MONTHS_PT = [
        "Jan",
        "Fev",
        "Mar",
        "Abr",
        "Mai",
        "Jun",
        "Jul",
        "Ago",
        "Set",
        "Out",
        "Nov",
        "Dez",
      ];

      // Dados representativos — dois anos consecutivos (2025 vs 2026).
      // Valores em centavos.
      const DEMO_2025 = [
        {
          income: 982000,
          economia: 245000,
          rate_bps: 2494,
        },
        // Jan 24,9%
        {
          income: 982000,
          economia: 310000,
          rate_bps: 3157,
        },
        // Fev 31,6%
        {
          income: 982000,
          economia: 198000,
          rate_bps: 2016,
        },
        // Mar 20,2%
        {
          income: 982000,
          economia: 176000,
          rate_bps: 1792,
        },
        // Abr 17,9%
        {
          income: 1074000,
          economia: 290000,
          rate_bps: 2700,
        },
        // Mai 27,0%
        {
          income: 982000,
          economia: 203000,
          rate_bps: 2067,
        },
        // Jun 20,7%
        {
          income: 982000,
          economia: 221000,
          rate_bps: 2251,
        },
        // Jul 22,5%
        {
          income: 982000,
          economia: 180000,
          rate_bps: 1834,
        },
        // Ago 18,3%
        {
          income: 982000,
          economia: 258000,
          rate_bps: 2628,
        },
        // Set 26,3%
        {
          income: 982000,
          economia: 195000,
          rate_bps: 1986,
        },
        // Out 19,9%
        {
          income: 982000,
          economia: 332000,
          rate_bps: 3381,
        },
        // Nov 33,8%
        {
          income: 1264000,
          economia: 341000,
          rate_bps: 2698,
        }, // Dez 27,0%
      ];
      const DEMO_2026 = [
        {
          income: 1050000,
          economia: 262000,
          rate_bps: 2495,
        },
        // Jan 25,0%
        {
          income: 1050000,
          economia: 340000,
          rate_bps: 3238,
        },
        // Fev 32,4%
        {
          income: 1050000,
          economia: 201000,
          rate_bps: 1914,
        },
        // Mar 19,1%
        {
          income: 1050000,
          economia: 284000,
          rate_bps: 2705,
        },
        // Abr 27,0%
        {
          income: 1050000,
          economia: 241000,
          rate_bps: 2295,
        },
        // Mai 23,0%
        {
          income: 1050000,
          economia: 189000,
          rate_bps: 1800,
        },
        // Jun 18,0% (mês em andamento)
        // Jul–Dez: meses futuros sem dados
        null,
        null,
        null,
        null,
        null,
        null,
      ];
      function yearTotals(months) {
        const filled = months.filter(Boolean);
        const totalIncome = filled.reduce((a, m) => a + m.income, 0);
        const totalEco = filled.reduce((a, m) => a + m.economia, 0);
        const savingsPct =
          totalIncome > 0 ? Math.round((totalEco / totalIncome) * 100) : 0;
        return {
          income: totalIncome,
          economia: totalEco,
          savingsPct,
        };
      }
      function savingsRateClass(pct) {
        if (pct > 30) return "ec-rate ec-rate--strong";
        if (pct >= SAVINGS_MIN_PCT) return "ec-rate ec-rate--ok";
        return "ec-rate ec-rate--warn";
      }

      /* ---- Sub-componente: 3 células de um mês ---- */
      function MonthCells({ m, leadingDivider, trailingDivider }) {
        const empty = !m;
        const pct = m ? Math.round(m.rate_bps / 100) : 0;
        const dim = empty ? 0.4 : 1;
        const entrTdClass = ["ec-td", leadingDivider ? "ec-td--divider" : ""]
          .filter(Boolean)
          .join(" ");
        const pctTdClass = ["ec-td", trailingDivider ? "ec-td--group-end" : ""]
          .filter(Boolean)
          .join(" ");
        return /*#__PURE__*/ React.createElement(
          React.Fragment,
          null,
          /*#__PURE__*/ React.createElement(
            "td",
            {
              className: entrTdClass,
              style: {
                opacity: dim,
              },
            },
            m
              ? /*#__PURE__*/ React.createElement(Money, {
                  cents: m.income,
                  size: "sm",
                  sign: "auto",
                })
              : /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    style: {
                      color: "var(--text-faint)",
                    },
                  },
                  "\u2014",
                ),
          ),
          /*#__PURE__*/ React.createElement(
            "td",
            {
              className: "ec-td",
              style: {
                opacity: dim,
              },
            },
            m
              ? /*#__PURE__*/ React.createElement(Money, {
                  cents: m.economia,
                  size: "sm",
                })
              : /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    style: {
                      color: "var(--text-faint)",
                    },
                  },
                  "\u2014",
                ),
          ),
          /*#__PURE__*/ React.createElement(
            "td",
            {
              className: pctTdClass,
              style: {
                opacity: dim,
              },
            },
            empty
              ? /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "ec-rate ec-rate--faint",
                  },
                  "\u2014",
                )
              : /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: savingsRateClass(pct),
                  },
                  pct,
                  "%",
                ),
          ),
        );
      }

      /* ---- Sub-componente: 3 células de total ---- */
      function TotalCells({ tot, leadingDivider, trailingDivider }) {
        const entrTdClass = ["ec-td", leadingDivider ? "ec-td--divider" : ""]
          .filter(Boolean)
          .join(" ");
        const pctTdClass = ["ec-td", trailingDivider ? "ec-td--group-end" : ""]
          .filter(Boolean)
          .join(" ");
        return /*#__PURE__*/ React.createElement(
          React.Fragment,
          null,
          /*#__PURE__*/ React.createElement(
            "td",
            {
              className: entrTdClass,
            },
            /*#__PURE__*/ React.createElement(Money, {
              cents: tot.income,
              size: "sm",
              sign: "auto",
            }),
          ),
          /*#__PURE__*/ React.createElement(
            "td",
            {
              className: "ec-td",
            },
            /*#__PURE__*/ React.createElement(Money, {
              cents: tot.economia,
              size: "sm",
            }),
          ),
          /*#__PURE__*/ React.createElement(
            "td",
            {
              className: pctTdClass,
              title:
                "Economizado anual = \u03A3Economia \xF7 \u03A3Entradas (meta 20\u201330%)",
            },
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: savingsRateClass(tot.savingsPct),
              },
              tot.savingsPct,
              "%",
            ),
          ),
        );
      }

      /* ---- Tela principal ---- */
      function EconomiaCompareScreen(props) {
        const [baseYear, setBaseYear] = React.useState(2025);
        const yearA = baseYear;
        const yearB = baseYear + 1;

        // Para a demo, só temos dados de 2025/2026.
        const monthsA = baseYear === 2025 ? DEMO_2025 : Array(12).fill(null);
        const monthsB = baseYear === 2025 ? DEMO_2026 : Array(12).fill(null);
        const totA = yearTotals(monthsA);
        const totB = yearTotals(monthsB);
        const hasAnyData = monthsA.some(Boolean) || monthsB.some(Boolean);
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "ec",
          },
          /*#__PURE__*/ React.createElement(
            "header",
            {
              className: "ec-header",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "ec-header__lead",
              },
              /*#__PURE__*/ React.createElement(
                "h1",
                {
                  className: "ec-title",
                },
                "Economia: ",
                yearA,
                " vs ",
                yearB,
              ),
              /*#__PURE__*/ React.createElement(
                "p",
                {
                  className: "ec-subtitle",
                },
                "Entradas, Economia e Economizado% m\xEAs a m\xEAs \u2014 dois anos lado a lado.",
              ),
            ),
            /*#__PURE__*/ React.createElement(MonthNav, {
              label: `${yearA} · ${yearB}`,
              onPrev: () => setBaseYear((y) => y - 1),
              onNext: () => setBaseYear((y) => y + 1),
              onToday: () => setBaseYear(2025),
              atToday: baseYear === 2025,
              prevLabel: "Par de anos anterior",
              nextLabel: "Pr\xF3ximo par de anos",
            }),
          ),
          !hasAnyData
            ? /*#__PURE__*/ React.createElement(EmptyState, {
                variant: "empty",
                title: "Sem dados de Economia",
                description:
                  "Importe a aba Economia em Configura\xE7\xF5es \u203A Google Sheets.",
              })
            : /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "dash-card",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "dash-card__body",
                    style: {
                      padding: 0,
                    },
                  },
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "ec-scroll",
                    },
                    /*#__PURE__*/ React.createElement(
                      "table",
                      {
                        className: "ec-table",
                        role: "table",
                        "aria-label": `Economia comparada ${yearA} vs ${yearB}`,
                      },
                      /*#__PURE__*/ React.createElement(
                        "thead",
                        null,
                        /*#__PURE__*/ React.createElement(
                          "tr",
                          null,
                          /*#__PURE__*/ React.createElement(
                            "th",
                            {
                              className: "ec-th ec-th--left",
                              rowSpan: 2,
                              scope: "col",
                              style: {
                                padding:
                                  "var(--space-3) var(--space-4) var(--space-3) var(--space-6)",
                              },
                            },
                            "M\xEAs",
                          ),
                          /*#__PURE__*/ React.createElement(
                            "th",
                            {
                              colSpan: 3,
                              className: "ec-th--year ec-th--year-a",
                              scope: "colgroup",
                            },
                            yearA,
                          ),
                          /*#__PURE__*/ React.createElement(
                            "th",
                            {
                              colSpan: 3,
                              className: "ec-th--year",
                              scope: "colgroup",
                            },
                            yearB,
                          ),
                        ),
                        /*#__PURE__*/ React.createElement(
                          "tr",
                          {
                            className: "ec-row--head",
                          },
                          /*#__PURE__*/ React.createElement(
                            "th",
                            {
                              className: "ec-th",
                              scope: "col",
                            },
                            "Entradas",
                          ),
                          /*#__PURE__*/ React.createElement(
                            "th",
                            {
                              className: "ec-th",
                              scope: "col",
                            },
                            "Economia",
                          ),
                          /*#__PURE__*/ React.createElement(
                            "th",
                            {
                              className: "ec-th ec-th--group-end",
                              scope: "col",
                            },
                            /*#__PURE__*/ React.createElement(
                              InfoPopover,
                              {
                                term: "economizado",
                              },
                              "Economizado",
                            ),
                          ),
                          /*#__PURE__*/ React.createElement(
                            "th",
                            {
                              className: "ec-th ec-th--divider",
                              scope: "col",
                            },
                            "Entradas",
                          ),
                          /*#__PURE__*/ React.createElement(
                            "th",
                            {
                              className: "ec-th",
                              scope: "col",
                            },
                            "Economia",
                          ),
                          /*#__PURE__*/ React.createElement(
                            "th",
                            {
                              className: "ec-th",
                              scope: "col",
                            },
                            /*#__PURE__*/ React.createElement(
                              InfoPopover,
                              {
                                term: "economizado",
                              },
                              "Economizado",
                            ),
                          ),
                        ),
                      ),
                      /*#__PURE__*/ React.createElement(
                        "tbody",
                        null,
                        MONTHS_PT.map((label, i) =>
                          /*#__PURE__*/ React.createElement(
                            "tr",
                            {
                              key: i,
                              className: "ec-row",
                            },
                            /*#__PURE__*/ React.createElement(
                              "td",
                              {
                                className: "ec-td--month",
                                style: {
                                  paddingLeft: "var(--space-6)",
                                },
                              },
                              label,
                            ),
                            /*#__PURE__*/ React.createElement(MonthCells, {
                              m: monthsA[i],
                              leadingDivider: false,
                              trailingDivider: true,
                            }),
                            /*#__PURE__*/ React.createElement(MonthCells, {
                              m: monthsB[i],
                              leadingDivider: true,
                              trailingDivider: false,
                            }),
                          ),
                        ),
                      ),
                      /*#__PURE__*/ React.createElement(
                        "tfoot",
                        null,
                        /*#__PURE__*/ React.createElement(
                          "tr",
                          {
                            className: "ec-row--foot",
                          },
                          /*#__PURE__*/ React.createElement(
                            "td",
                            {
                              className: "ec-td--total",
                              style: {
                                paddingLeft: "var(--space-6)",
                              },
                            },
                            "Total",
                          ),
                          /*#__PURE__*/ React.createElement(TotalCells, {
                            tot: totA,
                            leadingDivider: false,
                            trailingDivider: true,
                          }),
                          /*#__PURE__*/ React.createElement(TotalCells, {
                            tot: totB,
                            leadingDivider: true,
                            trailingDivider: false,
                          }),
                        ),
                      ),
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "ec-legend",
                      "aria-label": "Legenda de cores do Economizado%",
                    },
                    /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        className: "ec-legend__item",
                      },
                      /*#__PURE__*/ React.createElement("span", {
                        className: "ec-legend__dot",
                        style: {
                          background: "var(--primary)",
                        },
                        "aria-hidden": "true",
                      }),
                      "> 30% — acima da meta",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        className: "ec-legend__item",
                      },
                      /*#__PURE__*/ React.createElement("span", {
                        className: "ec-legend__dot",
                        style: {
                          background: "var(--success-400)",
                        },
                        "aria-hidden": "true",
                      }),
                      "20–30% — dentro do ideal",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        className: "ec-legend__item",
                      },
                      /*#__PURE__*/ React.createElement("span", {
                        className: "ec-legend__dot",
                        style: {
                          background: "var(--warning-400)",
                        },
                        "aria-hidden": "true",
                      }),
                      "< 20% — abaixo da meta",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        style: {
                          marginLeft: "auto",
                          color: "var(--text-faint)",
                        },
                      },
                      "Economizado% = Economia \xF7 Entradas (meta 20\u201330%)",
                    ),
                  ),
                ),
              ),
        );
      }
      window.EconomiaCompareScreen = EconomiaCompareScreen;
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "ui_kits/economia-compare/EconomiaCompareScreen.jsx",
      error: String((e && e.message) || e),
    });
  }

  // ui_kits/horizonte/HorizonteScreen.jsx
  try {
    (() => {
      /* Neko Finance — Horizonte de saldos (ui_kit).
         Projeção mês a mês do saldo — termômetro visual de folga/aperto.
         Seções: gráfico BalanceTrajectory + legenda de faixas, detalhe diário por mês
         (colunas do calendário com heatmap de saldo), vencimentos próximos.
         PT-BR copy · R$ em mono tabular · zero dependências externas.
         Expõe window.HorizonteScreen. */

      const NS = window.NekoFinanceDesignSystem_9bd1cd;
      const { Money, BalanceTrajectory, ProvBadge } = NS;
      const Icon = window.Icon;

      /* ---- CSS (once-only) ---- */
      (function injectHorizonteCSS() {
        if (document.getElementById("horizonte-css")) return;
        const s = document.createElement("style");
        s.id = "horizonte-css";
        s.textContent = `
      /* Layout geral */
      .hor { display: flex; flex-direction: column; gap: var(--space-6); padding: var(--space-2); max-width: 1200px; }

      /* Cabeçalho */
      .hor-head__title {
        font-size: var(--fs-h2);
        font-weight: var(--fw-bold);
        letter-spacing: var(--ls-tight);
        margin: 0 0 var(--space-2);
        color: var(--text-strong);
      }
      .hor-head__desc {
        font-size: var(--fs-sm);
        color: var(--text-muted);
        margin: 0;
        line-height: var(--lh-normal);
        max-width: 560px;
      }

      /* Card do gráfico */
      .hor-chart-card {
        background: var(--surface);
        border: var(--bw-hair) solid var(--border);
        border-radius: var(--radius-lg);
        box-shadow: var(--shadow-1);
        padding: var(--space-6) var(--space-6) var(--space-4);
      }
      .hor-chart-card__legend {
        display: flex;
        flex-wrap: wrap;
        gap: var(--space-4);
        margin-top: var(--space-3);
        padding-top: var(--space-3);
        border-top: var(--bw-hair) solid var(--border);
      }
      .hor-legend-item {
        display: inline-flex;
        align-items: center;
        gap: 7px;
        font-size: var(--fs-sm);
        color: var(--text-muted);
      }
      .hor-legend-swatch {
        width: 12px;
        height: 12px;
        border-radius: var(--radius-xs);
        flex-shrink: 0;
      }

      /* Seção de etiqueta */
      .hor-section-label {
        font-size: var(--fs-label);
        font-weight: var(--fw-semibold);
        letter-spacing: var(--ls-label);
        text-transform: uppercase;
        color: var(--text-faint);
        margin: 0 0 var(--space-3);
      }

      /* Colunas mensais de detalhe diário */
      .hor-cols {
        display: flex;
        gap: var(--space-4);
        overflow-x: auto;
        padding-bottom: var(--space-4);
        -webkit-overflow-scrolling: touch;
      }
      .hor-col { min-width: 140px; flex-shrink: 0; }
      .hor-col__month {
        font-size: var(--fs-label);
        font-weight: var(--fw-bold);
        letter-spacing: var(--ls-label);
        text-transform: uppercase;
        color: var(--text-muted);
        padding: var(--space-2) var(--space-3);
        position: sticky;
        top: 0;
        background: var(--bg);
      }
      .hor-col__list {
        display: flex;
        flex-direction: column;
        gap: 2px;
        list-style: none;
        margin: 0;
        padding: 0;
      }
      .hor-day {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-3);
        padding: var(--space-2) var(--space-3);
        border-radius: var(--radius-sm);
        font-variant-numeric: tabular-nums;
        transition: outline 80ms ease;
      }
      .hor-day--today { outline: 2px solid var(--border-focus); }
      .hor-day__num {
        font-size: var(--fs-sm);
        color: var(--text);
        width: 22px;
        flex-shrink: 0;
      }
      .hor-day__bal {
        color: var(--text);
        font-size: var(--fs-sm);
      }

      /* Vencimentos */
      .hor-bills-title {
        font-size: var(--fs-label);
        font-weight: var(--fw-semibold);
        letter-spacing: var(--ls-label);
        text-transform: uppercase;
        color: var(--text-faint);
        margin: var(--space-6) 0 var(--space-3);
      }
      .hor-bills-list {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        list-style: none;
        margin: 0;
        padding: 0;
      }
      .hor-bill {
        display: flex;
        align-items: center;
        gap: var(--space-3);
        padding: var(--space-3);
        background: var(--surface);
        border: var(--bw-hair) solid var(--border);
        border-radius: var(--radius-sm);
      }
      .hor-bill__chip {
        display: inline-flex;
        align-items: center;
        gap: 5px;
        padding: 2px 8px;
        border-radius: var(--radius-pill);
        background: var(--surface-2);
        border: var(--bw-hair) solid var(--border);
        font-size: var(--fs-micro);
        font-weight: var(--fw-medium);
        color: var(--text-muted);
        white-space: nowrap;
        flex-shrink: 0;
      }
      .hor-bill__desc {
        flex: 1;
        font-size: var(--fs-sm);
        color: var(--text);
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }

      /* Empty state inline (vencimentos vazios) */
      .hor-empty {
        padding: var(--space-7) var(--space-6);
        background: var(--bg-subtle);
        border: var(--bw-hair) solid var(--border);
        border-radius: var(--radius-sm);
        text-align: center;
        color: var(--text-faint);
        font-size: var(--fs-sm);
      }

      @media (prefers-reduced-motion: reduce) {
        .hor-day { transition: none; }
      }
      `;
        document.head.appendChild(s);
      })();

      /* ---- Dados de demo representativos ---- */

      /** Calcula a faixa de saldo (limiares absolutos da planilha, em centavos). */
      function saldoBand(cents) {
        if (cents < -50000) return "critical"; // < −R$ 500
        if (cents < 0) return "negative"; // < R$ 0
        if (cents <= 100000) return "tight"; // ≤ R$ 1.000
        if (cents <= 200000) return "ok"; // ≤ R$ 2.000
        return "comfortable"; // > R$ 2.000
      }
      const BAND_FILL = {
        critical: "var(--saldo-band-critical-fill)",
        negative: "var(--saldo-band-negative-fill)",
        tight: "var(--saldo-band-tight-fill)",
        ok: "var(--saldo-band-ok-fill)",
        comfortable: "var(--saldo-band-comfortable-fill)",
      };
      const BAND_LEGEND = [
        {
          band: "comfortable",
          label: "folga (> R$ 2.000)",
        },
        {
          band: "ok",
          label: "ok (R$ 1.000–2.000)",
        },
        {
          band: "tight",
          label: "apertado (R$ 0–1.000)",
        },
        {
          band: "negative",
          label: "negativo (−R$ 500 a R$ 0)",
        },
        {
          band: "critical",
          label: "crítico (< −R$ 500)",
        },
      ];

      /** Meses fictícios PT-BR abreviados. */
      const MONTH_NAMES = [
        "",
        "jan",
        "fev",
        "mar",
        "abr",
        "mai",
        "jun",
        "jul",
        "ago",
        "set",
        "out",
        "nov",
        "dez",
      ];
      function monthLabel(ym) {
        const [, m] = ym.split("-");
        const n = MONTH_NAMES[Number(m)];
        return n.charAt(0).toUpperCase() + n.slice(1);
      }

      /** Série diária de 3 meses (jun–ago 2026) com trajetória realista.
          Saldo começa confortável, aperta em julho, recupera em agosto. */
      function buildDemoDaily() {
        const days = [];
        const months = [
          {
            ym: "2026-06",
            start: 315000,
            perDay: -5800,
          },
          // R$ 3.150 → declínio suave
          {
            ym: "2026-07",
            start: 145000,
            perDay: -6200,
          },
          // R$ 1.450 → faixa apertada/negativa
          {
            ym: "2026-08",
            start: 280000,
            perDay: -4500,
          }, // R$ 2.800 → salário nova entrada
        ];
        for (const { ym, start, perDay } of months) {
          const [year, month] = ym.split("-").map(Number);
          const daysInMonth = new Date(year, month, 0).getDate();
          for (let d = 1; d <= daysInMonth; d++) {
            const date = `${ym}-${String(d).padStart(2, "0")}`;
            // Injetar salário no dia 5 de jul e ago
            const payday = d === 5 && month > 6;
            const balance = start + (d - 1) * perDay + (payday ? 620000 : 0);
            days.push({
              date,
              balance_cents: Math.round(balance),
            });
          }
        }
        return days;
      }
      const DEMO_DAILY = buildDemoDaily();
      const DEMO_TODAY = "2026-06-21";

      /** Agrupa a série diária em colunas por mês. */
      function groupByMonth(daily, today) {
        const colsMap = new Map();
        const colsOrder = [];
        for (const d of daily) {
          const ym = d.date.slice(0, 7);
          if (!colsMap.has(ym)) {
            const col = {
              ym,
              label: monthLabel(ym),
              days: [],
            };
            colsMap.set(ym, col);
            colsOrder.push(col);
          }
          colsMap.get(ym).days.push({
            day: Number(d.date.slice(8, 10)),
            balance: d.balance_cents,
            isToday: d.date === today,
          });
        }
        return colsOrder;
      }

      /** Formata data "DD/MM" para o chip de vencimento. */
      function fmtDate(iso) {
        const [, m, d] = iso.split("-");
        return `${d}/${m}`;
      }

      /* ---- Sub-componentes ---- */

      function ChartSection() {
        return /*#__PURE__*/ React.createElement(
          "section",
          {
            className: "hor-chart-card",
            "aria-label": "Trajet\xF3ria do saldo projetado",
          },
          /*#__PURE__*/ React.createElement(BalanceTrajectory, {
            daily: DEMO_DAILY,
            today: DEMO_TODAY,
            variant: "full",
          }),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "hor-chart-card__legend",
              "aria-label": "Legenda das faixas de saldo",
            },
            BAND_LEGEND.map((l) =>
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  key: l.band,
                  className: "hor-legend-item",
                },
                /*#__PURE__*/ React.createElement("span", {
                  "aria-hidden": "true",
                  className: "hor-legend-swatch",
                  style: {
                    background: BAND_FILL[l.band],
                  },
                }),
                l.label,
              ),
            ),
          ),
        );
      }
      function DailyDetail() {
        const cols = groupByMonth(DEMO_DAILY, DEMO_TODAY);
        return /*#__PURE__*/ React.createElement(
          "section",
          {
            "aria-label": "Saldo projetado por dia, agrupado por m\xEAs",
          },
          /*#__PURE__*/ React.createElement(
            "h2",
            {
              className: "hor-section-label",
            },
            "Detalhe di\xE1rio",
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "hor-cols",
            },
            cols.map((col) =>
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  key: col.ym,
                  className: "hor-col",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    "aria-hidden": "true",
                    className: "hor-col__month",
                  },
                  col.label,
                ),
                /*#__PURE__*/ React.createElement(
                  "ul",
                  {
                    "aria-label": col.label,
                    className: "hor-col__list",
                  },
                  col.days.map((d) => {
                    const band = saldoBand(d.balance);
                    return /*#__PURE__*/ React.createElement(
                      "li",
                      {
                        key: d.day,
                        "aria-current": d.isToday ? "date" : undefined,
                        "aria-label": `Dia ${d.day}: faixa ${BAND_LEGEND.find((l) => l.band === band)?.label ?? band}`,
                        className: `hor-day${d.isToday ? " hor-day--today" : ""}`,
                        style: {
                          background: BAND_FILL[band],
                        },
                      },
                      /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          "aria-hidden": "true",
                          className: "hor-day__num",
                        },
                        d.day,
                      ),
                      /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          "aria-hidden": "true",
                          className: "hor-day__bal",
                        },
                        /*#__PURE__*/ React.createElement(Money, {
                          cents: d.balance,
                          size: "sm",
                          sign: "none",
                        }),
                      ),
                    );
                  }),
                ),
              ),
            ),
          ),
        );
      }

      /** Vencimentos próximos — demo com 4 contas a pagar nos próximos 60 dias. */
      function UpcomingBills() {
        const bills = [
          {
            id: 1,
            due_date: "2026-06-25",
            description: "Aluguel",
            amount: 180000,
            is_projection: false,
          },
          {
            id: 2,
            due_date: "2026-06-28",
            description: "Fatura do cartão",
            amount: 243700,
            is_projection: false,
          },
          {
            id: 3,
            due_date: "2026-07-05",
            description: "Plano de saúde",
            amount: 58900,
            is_projection: true,
          },
          {
            id: 4,
            due_date: "2026-07-10",
            description: "Internet + streaming",
            amount: 18490,
            is_projection: true,
          },
        ];
        return /*#__PURE__*/ React.createElement(
          "section",
          {
            "aria-labelledby": "hor-bills-title",
          },
          /*#__PURE__*/ React.createElement(
            "h2",
            {
              id: "hor-bills-title",
              className: "hor-bills-title",
            },
            "Vencimentos pr\xF3ximos",
          ),
          /*#__PURE__*/ React.createElement(
            "ul",
            {
              className: "hor-bills-list",
            },
            bills.map((b) =>
              /*#__PURE__*/ React.createElement(
                "li",
                {
                  key: b.id,
                  className: "hor-bill",
                },
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "hor-bill__chip",
                  },
                  /*#__PURE__*/ React.createElement(Icon, {
                    name: "calendar",
                    size: 12,
                    stroke: 1.75,
                    "aria-hidden": "true",
                  }),
                  fmtDate(b.due_date),
                ),
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "hor-bill__desc",
                  },
                  b.description,
                ),
                b.is_projection &&
                  /*#__PURE__*/ React.createElement(ProvBadge, {
                    provenance: "projetado",
                  }),
                /*#__PURE__*/ React.createElement(Money, {
                  cents: -Math.abs(b.amount),
                  size: "sm",
                  sign: "auto",
                }),
              ),
            ),
          ),
        );
      }

      /* ---- Tela completa ---- */
      function HorizonteScreen(props) {
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "hor",
          },
          /*#__PURE__*/ React.createElement(
            "header",
            null,
            /*#__PURE__*/ React.createElement(
              "h1",
              {
                className: "hor-head__title",
              },
              "Horizonte de saldos",
            ),
            /*#__PURE__*/ React.createElement(
              "p",
              {
                className: "hor-head__desc",
              },
              "Saldo projetado dia a dia, no mesmo term\xF4metro da planilha: quanto mais verde, mais folga; quanto mais vermelho, mais aperto.",
            ),
          ),
          /*#__PURE__*/ React.createElement(ChartSection, null),
          /*#__PURE__*/ React.createElement(DailyDetail, null),
          /*#__PURE__*/ React.createElement(UpcomingBills, null),
        );
      }
      window.HorizonteScreen = HorizonteScreen;
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "ui_kits/horizonte/HorizonteScreen.jsx",
      error: String((e && e.message) || e),
    });
  }

  // ui_kits/methodology/MethodologyScreen.jsx
  try {
    (() => {
      /* Neko Finance — Ajuda / princípios do método. Lista de 7 cards de princípio
         com hero intro. Expõe window.MethodologyScreen. */
      const NS = window.NekoFinanceDesignSystem_9bd1cd;
      const { Badge } = NS;
      const Icon = window.Icon;
      const metCSS = `
      .met{display:flex;flex-direction:column;gap:20px;max-width:1080px;}
      .met-hero{padding:18px 20px;background:var(--surface);border:1px solid var(--border);
        border-radius:var(--radius-lg);box-shadow:var(--shadow-1);}
      .met-hero__eyebrow{font-size:11px;font-weight:700;letter-spacing:.08em;text-transform:uppercase;
        color:var(--primary);margin-bottom:8px;}
      .met-hero__line{font-size:15px;line-height:1.55;color:var(--text-muted);}
      .met-hero__line b{color:var(--text-strong);font-weight:700;}
      .met-grid{display:grid;grid-template-columns:repeat(3,1fr);gap:14px;}
      .met-card{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);
        box-shadow:var(--shadow-1);padding:18px 18px 20px;display:flex;flex-direction:column;gap:10px;}
      .met-card__ic{width:34px;height:34px;border-radius:var(--radius-sm);
        background:var(--primary-quiet);color:var(--primary);
        display:flex;align-items:center;justify-content:center;flex:none;}
      .met-card__title{font-size:14px;font-weight:700;color:var(--text-strong);line-height:1.3;
        letter-spacing:-0.01em;}
      .met-card__body{font-size:13.5px;line-height:1.6;color:var(--text-muted);flex:1;}
      .met-card__body b{color:var(--text);font-weight:600;}
      .met-card__body em{font-style:italic;color:var(--text);}
      @media (max-width:960px){.met-grid{grid-template-columns:repeat(2,1fr);}}
      @media (max-width:600px){.met-grid{grid-template-columns:1fr;}}
      @media (prefers-reduced-motion:reduce){*{transition:none!important;animation:none!important;}}
      `;
      function injectMet() {
        if (document.getElementById("met-css")) return;
        const s = document.createElement("style");
        s.id = "met-css";
        s.textContent = metCSS;
        document.head.appendChild(s);
      }
      const PRINCIPLES = [
        {
          icon: "trendingUp",
          title: "Saldo projetado, não saldo atual",
          body: /*#__PURE__*/ React.createElement(
            React.Fragment,
            null,
            "A pergunta que importa n\xE3o \xE9 ",
            /*#__PURE__*/ React.createElement("em", null, '"quanto eu tenho?"'),
            ", e sim",
            " ",
            /*#__PURE__*/ React.createElement("em", null, '"quanto vai sobrar?"'),
            ". O Neko encadeia dia a dia as entradas e sa\xEDdas futuras e mostra o ",
            /*#__PURE__*/ React.createElement("b", null, "saldo projetado"),
            " para o fim do m\xEAs: esse \xE9 o n\xFAmero her\xF3i do dashboard.",
          ),
        },
        {
          icon: "sliders",
          title: "A conta do mês (Performance)",
          body: /*#__PURE__*/ React.createElement(
            React.Fragment,
            null,
            "Performance = Entradas \u2212 (Sa\xEDdas + Di\xE1rio + Economia + previs\xE3o do di\xE1rio que ainda falta). As Sa\xEDdas j\xE1 incluem as contas fixas e a ",
            /*#__PURE__*/ React.createElement("b", null, "fatura do cart\xE3o"),
            " \u2014 que entra como sa\xEDda no vencimento, sem coluna pr\xF3pria. Por isso o m\xEAs nasce no vermelho e vai esverdeando conforme o di\xE1rio real fica abaixo do teto.",
          ),
        },
        {
          icon: "calendarRange",
          title: "Custo de vida",
          body: /*#__PURE__*/ React.createElement(
            React.Fragment,
            null,
            "Custo de vida = ",
            /*#__PURE__*/ React.createElement("b", null, "Sa\xEDdas"),
            " (contas fixas previs\xEDveis + fatura do cart\xE3o no vencimento) + ",
            /*#__PURE__*/ React.createElement("b", null, "Di\xE1rio"),
            " (o resto). O di\xE1rio \xE9 um n\xFAmero \xFAnico por dia, n\xE3o um or\xE7amento por categoria: categorias servem para diagn\xF3stico,",
            " ",
            /*#__PURE__*/ React.createElement("em", null, "nunca para planejamento"),
            ".",
          ),
        },
        {
          icon: "piggy",
          title: "Guardar 20 a 30%",
          body: /*#__PURE__*/ React.createElement(
            React.Fragment,
            null,
            "Economizado = o quanto voc\xEA transfere para a reserva \xF7 entradas. A meta \xE9",
            " ",
            /*#__PURE__*/ React.createElement("b", null, "20 a 30%"),
            " \u2014 mas como ",
            /*#__PURE__*/ React.createElement("em", null, "m\xE9dia do ano"),
            ", n\xE3o de cada m\xEAs (uns meses mais, outros menos). \xC9 diferente do colch\xE3o: a Economia \xE9 o que voc\xEA separa de prop\xF3sito.",
          ),
        },
        {
          icon: "creditCard",
          title: "Débito e crédito: dois ritmos",
          body: /*#__PURE__*/ React.createElement(
            React.Fragment,
            null,
            "D\xE9bito, PIX e dinheiro afetam o caixa no mesmo dia. O cr\xE9dito \xE9 diferente: cada compra vai para a fatura e o Neko lan\xE7a esse total como uma",
            " ",
            /*#__PURE__*/ React.createElement(
              "b",
              null,
              "Sa\xEDda \xFAnica no vencimento",
            ),
            " \u2014 o cart\xE3o sequestra o sal\xE1rio futuro. Por isso a fatura aparece nas Sa\xEDdas, n\xE3o no Di\xE1rio.",
          ),
        },
        {
          icon: "shield",
          title: "Reserva em meses",
          body: /*#__PURE__*/ React.createElement(
            React.Fragment,
            null,
            "A reserva de emerg\xEAncia \xE9 medida em ",
            /*#__PURE__*/ React.createElement("b", null, "meses de custo de vida"),
            " (reserva \xF7 custo mensal), n\xE3o em valor absoluto. A meta m\xEDnima \xE9 ",
            /*#__PURE__*/ React.createElement("b", null, "6 meses"),
            "; a partir de 12 \xE9 a paz financeira, e o excedente pode trabalhar em outro lugar.",
          ),
        },
        {
          icon: "calculator",
          title: "Cálculo determinístico",
          body: /*#__PURE__*/ React.createElement(
            React.Fragment,
            null,
            "Todos os n\xFAmeros v\xEAm de um ",
            /*#__PURE__*/ React.createElement(
              "b",
              null,
              "motor de c\xE1lculo determin\xEDstico",
            ),
            " e testado. A Mia (em desenvolvimento) vai explicar e contextualizar esses n\xFAmeros sem nunca inventar contas; e nenhuma escrita na sua planilha acontece sem a sua",
            " ",
            /*#__PURE__*/ React.createElement(
              "em",
              null,
              "aprova\xE7\xE3o expl\xEDcita",
            ),
            ".",
          ),
        },
      ];
      function MethodologyScreen() {
        injectMet();
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "met",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "met-hero",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "met-hero__eyebrow",
              },
              "Ajuda",
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "met-hero__line",
              },
              /*#__PURE__*/ React.createElement("b", null, "Previsibilidade primeiro."),
              " O Neko organiza suas finan\xE7as em torno de uma \xFAnica disciplina: saber hoje como o m\xEAs termina. Os sete princ\xEDpios abaixo explicam como cada n\xFAmero \xE9 calculado.",
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "met-grid",
            },
            PRINCIPLES.map((p) =>
              /*#__PURE__*/ React.createElement(
                "article",
                {
                  className: "met-card",
                  key: p.title,
                },
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "met-card__ic",
                  },
                  /*#__PURE__*/ React.createElement(Icon, {
                    name: p.icon,
                    size: 18,
                    stroke: 1.75,
                  }),
                ),
                /*#__PURE__*/ React.createElement(
                  "h2",
                  {
                    className: "met-card__title",
                  },
                  p.title,
                ),
                /*#__PURE__*/ React.createElement(
                  "p",
                  {
                    className: "met-card__body",
                  },
                  p.body,
                ),
              ),
            ),
          ),
        );
      }
      window.MethodologyScreen = MethodologyScreen;
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "ui_kits/methodology/MethodologyScreen.jsx",
      error: String((e && e.message) || e),
    });
  }

  // ui_kits/settings/SettingsScreen.jsx
  try {
    (() => {
      /* Neko Finance — Configurações e privacidade. Fiel ao SettingsScreen.tsx de produção:
         conexão Google Sheets, importação .xlsx local, bolsos (Pockets), lembrete diário,
         teto do Diário, categorias do Diário e seus dados (local-first, backup, versão).
         Expõe window.SettingsScreen — o index.html envolve com window.AppShell. */
      const NS = window.NekoFinanceDesignSystem_9bd1cd;
      const { SegmentedControl, Button, Badge } = NS;
      const Icon = window.Icon;
      (function injectSettingsCSS() {
        if (document.getElementById("settings-css")) return;
        const s = document.createElement("style");
        s.id = "settings-css";
        s.textContent = `
      .set{max-width:760px;margin:0 auto;display:flex;flex-direction:column;gap:28px;}

      /* cabeçalho de seção */
      .set-sec__head{margin-bottom:11px;}
      .set-sec__title{font-size:15px;font-weight:700;color:var(--text-strong);letter-spacing:-0.005em;
        display:flex;align-items:center;gap:9px;margin:0;}
      .set-sec__ic{color:var(--text-faint);flex:none;}
      .set-sec__sub{font-size:12.5px;color:var(--text-muted);margin:3px 0 0 26px;line-height:1.45;}

      /* painel de cartão */
      .set-panel{background:var(--surface);border:1px solid var(--border);
        border-radius:var(--radius-md);box-shadow:var(--shadow-1);overflow:hidden;}
      .set-panel--pad{padding:0;}

      /* linha de configuração */
      .set-row{display:flex;align-items:center;gap:14px;padding:14px 16px;
        border-bottom:1px solid var(--border);}
      .set-row:last-child{border-bottom:none;}
      .set-row__main{flex:1;min-width:0;}
      .set-row__t{font-size:13.5px;font-weight:600;color:var(--text);}
      .set-row__d{font-size:12px;color:var(--text-muted);margin-top:2px;line-height:1.45;}
      .set-row__d code{font-family:var(--font-mono);font-size:11px;background:var(--surface-2);
        padding:1px 5px;border-radius:4px;color:var(--text);}
      .set-row__ctl{flex:none;display:flex;align-items:center;gap:8px;}

      /* bloco de conexão com logo */
      .set-conn{display:flex;align-items:center;gap:12px;padding:15px 16px;
        background:var(--bg-subtle);border-bottom:1px solid var(--border);}
      .set-conn__logo{width:38px;height:38px;border-radius:10px;background:var(--surface);
        border:1px solid var(--border);display:flex;align-items:center;justify-content:center;
        color:var(--success-500);flex:none;}
      .set-conn__t{font-size:14px;font-weight:700;color:var(--text-strong);}
      .set-conn__s{font-size:12px;color:var(--text-muted);margin-top:2px;
        display:flex;align-items:center;gap:6px;}
      .set-conn__dot{width:6px;height:6px;border-radius:50%;display:inline-block;flex:none;}
      .set-conn__dot--ok{background:var(--success-500);}
      .set-conn__dot--off{background:var(--text-faint);}

      /* bolsos (pockets) */
      .set-pockets{display:flex;flex-direction:column;}
      .set-pocket-row{display:flex;align-items:center;gap:10px;padding:12px 16px;
        border-bottom:1px solid var(--border);}
      .set-pocket-row:last-child{border-bottom:none;}
      .set-pocket__ic{width:32px;height:32px;border-radius:8px;background:var(--surface-elevated);
        border:1px solid var(--border);display:flex;align-items:center;justify-content:center;
        color:var(--text-muted);flex:none;}
      .set-pocket__nm{font-size:13px;font-weight:600;color:var(--text);}
      .set-pocket__sub{font-size:11px;color:var(--text-faint);}
      .set-pocket__amt{margin-left:auto;font-family:var(--font-money);font-variant-numeric:tabular-nums;
        font-size:14px;font-weight:600;color:var(--text);}
      .set-pocket__badge{flex:none;}

      /* categorias do Diário */
      .set-cats{padding:14px 16px;}
      .set-cat-row{display:flex;align-items:center;gap:8px;margin-bottom:8px;}
      .set-cat-row:last-child{margin-bottom:0;}
      .set-cat__name{flex:1;font-size:13px;font-weight:500;color:var(--text);}
      .set-cat__amt{font-family:var(--font-money);font-variant-numeric:tabular-nums;
        font-size:13px;color:var(--text-muted);min-width:80px;text-align:right;}
      .set-cat__bar{height:4px;border-radius:2px;background:var(--surface-2);margin-top:4px;overflow:hidden;}
      .set-cat__fill{height:100%;border-radius:2px;background:var(--primary);}
      .set-cats__total{margin-top:10px;padding-top:10px;border-top:1px solid var(--border);
        display:flex;justify-content:space-between;font-size:12px;color:var(--text-muted);}
      .set-cats__total-amt{font-family:var(--font-money);font-variant-numeric:tabular-nums;
        font-weight:600;color:var(--text);}

      /* zona de perigo */
      .set-danger{border-color:color-mix(in srgb,var(--danger-500) 30%,var(--border));}
      .set-danger .set-row__t{color:var(--danger-400);}

      /* meta da versão */
      .set-meta{display:flex;align-items:center;gap:6px;font-size:11.5px;
        color:var(--text-faint);font-family:var(--font-mono);}

      /* lembrete: campo de horário */
      .set-time-ctl{display:flex;align-items:center;gap:8px;}
      .set-time-input{font-family:var(--font-money);font-size:var(--fs-body);
        background:var(--bg-subtle);border:1px solid var(--border-input);
        border-radius:var(--radius-xs);color:var(--text);padding:4px 8px;
        height:var(--hit-min);}
      `;
        document.head.appendChild(s);
      })();

      /* ---- Componentes de suporte ---- */

      function Section({ icon, title, sub, children }) {
        return /*#__PURE__*/ React.createElement(
          "section",
          null,
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "set-sec__head",
            },
            /*#__PURE__*/ React.createElement(
              "h2",
              {
                className: "set-sec__title",
              },
              /*#__PURE__*/ React.createElement(Icon, {
                name: icon,
                size: 17,
                className: "set-sec__ic",
              }),
              title,
            ),
            sub
              ? /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-sec__sub",
                  },
                  sub,
                )
              : null,
          ),
          children,
        );
      }

      /* ---- Seção: Conexão Google Sheets ---- */
      function ConexaoSection() {
        const [status, setStatus] = React.useState("connected"); // connected | disconnected | expired

        return /*#__PURE__*/ React.createElement(
          Section,
          {
            icon: "link",
            title: "Conex\xE3o Google Sheets",
            sub: "O Neko l\xEA sua planilha. Nada \xE9 escrito sem a sua aprova\xE7\xE3o.",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "set-panel",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "set-conn",
              },
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "set-conn__logo",
                },
                /*#__PURE__*/ React.createElement(Icon, {
                  name: "table",
                  size: 19,
                }),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  style: {
                    flex: 1,
                  },
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-conn__t",
                  },
                  "Google Sheets",
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-conn__s",
                  },
                  /*#__PURE__*/ React.createElement("span", {
                    className:
                      "set-conn__dot " +
                      (status === "connected"
                        ? "set-conn__dot--ok"
                        : "set-conn__dot--off"),
                    "aria-hidden": "true",
                  }),
                  status === "connected"
                    ? "voce@gmail.com · somente leitura"
                    : status === "expired"
                      ? "Sessão expirada — reconecte para sincronizar"
                      : "Desconectado",
                ),
              ),
              status === "connected"
                ? /*#__PURE__*/ React.createElement(
                    Badge,
                    {
                      tone: "success",
                      dot: true,
                    },
                    "Conectado",
                  )
                : /*#__PURE__*/ React.createElement(
                    Badge,
                    {
                      tone: "warning",
                    },
                    "Desconectado",
                  ),
              /*#__PURE__*/ React.createElement(
                Button,
                {
                  variant: "secondary",
                  size: "sm",
                  onClick: () =>
                    setStatus(status === "connected" ? "disconnected" : "connected"),
                },
                status === "connected" ? "Reconectar" : "Conectar",
              ),
            ),
            status === "connected"
              ? /*#__PURE__*/ React.createElement(
                  React.Fragment,
                  null,
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "set-row",
                    },
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__main",
                      },
                      /*#__PURE__*/ React.createElement(
                        "div",
                        {
                          className: "set-row__t",
                        },
                        "Planilha ativa",
                      ),
                      /*#__PURE__*/ React.createElement(
                        "div",
                        {
                          className: "set-row__d",
                        },
                        "Pasta de trabalho ",
                        /*#__PURE__*/ React.createElement(
                          "code",
                          null,
                          "Finan\xE7as 2025",
                        ),
                        " \xB7 226 lan\xE7amentos \xB7 sincronizada h\xE1 3 min",
                      ),
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__ctl",
                      },
                      /*#__PURE__*/ React.createElement(
                        Button,
                        {
                          variant: "ghost",
                          size: "sm",
                          iconLeft: /*#__PURE__*/ React.createElement(Icon, {
                            name: "refresh",
                            size: 14,
                          }),
                        },
                        "Re-sincronizar",
                      ),
                      /*#__PURE__*/ React.createElement(
                        Button,
                        {
                          variant: "ghost",
                          size: "sm",
                        },
                        "Trocar",
                      ),
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "set-row",
                    },
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__main",
                      },
                      /*#__PURE__*/ React.createElement(
                        "div",
                        {
                          className: "set-row__t",
                        },
                        "Escrita na planilha",
                      ),
                      /*#__PURE__*/ React.createElement(
                        "div",
                        {
                          className: "set-row__d",
                        },
                        "O Neko prop\xF5e edi\xE7\xF5es como um diff. Nada \xE9 gravado at\xE9 voc\xEA aprovar.",
                      ),
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__ctl",
                      },
                      /*#__PURE__*/ React.createElement(
                        Badge,
                        {
                          tone: "primary",
                        },
                        "Aprova\xE7\xE3o obrigat\xF3ria",
                      ),
                    ),
                  ),
                )
              : null,
          ),
        );
      }

      /* ---- Seção: Importar arquivo local ---- */
      function ImportacaoLocalSection() {
        const [imported, setImported] = React.useState(false);
        return /*#__PURE__*/ React.createElement(
          Section,
          {
            icon: "download",
            title: "Importar arquivo local",
            sub: "Use uma c\xF3pia .xlsx da planilha quando n\xE3o quiser conectar a conta Google.",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "set-panel",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "set-row",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__main",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__t",
                  },
                  "Planilha .xlsx",
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__d",
                  },
                  "Importa todas as abas, detectando o layout de blocos mensais automaticamente. Linhas j\xE1 importadas antes s\xE3o ignoradas.",
                  imported
                    ? /*#__PURE__*/ React.createElement(
                        "strong",
                        null,
                        " Importado com sucesso.",
                      )
                    : null,
                ),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__ctl",
                },
                /*#__PURE__*/ React.createElement(
                  Button,
                  {
                    variant: "secondary",
                    size: "sm",
                    iconLeft: /*#__PURE__*/ React.createElement(Icon, {
                      name: "download",
                      size: 14,
                    }),
                    onClick: () => setImported(true),
                  },
                  "Escolher arquivo\u2026",
                ),
              ),
            ),
          ),
        );
      }

      /* ---- Seção: Bolsos ---- */
      const BOLSOS = [
        {
          nm: "Conta corrente",
          sub: "Banco Aurora ·· 4821",
          amt: "R$ 12.408,52",
          ic: "wallet",
          liquid: true,
        },
        {
          nm: "Poupança",
          sub: "Caixa ·· 9920",
          amt: "R$ 5.800,00",
          ic: "piggy",
          liquid: true,
        },
        {
          nm: "Vale-alimentação",
          sub: "Flash · cartão VA",
          amt: "R$ 620,00",
          ic: "creditCard",
          liquid: true,
        },
        {
          nm: "FGTS",
          sub: "Caixa · bloqueado",
          amt: "R$ 38.410,00",
          ic: "lock",
          liquid: false,
        },
        {
          nm: "Previdência",
          sub: "XP · longo prazo",
          amt: "R$ 22.900,00",
          ic: "shield",
          liquid: false,
        },
      ];
      function BolsosSection() {
        return /*#__PURE__*/ React.createElement(
          Section,
          {
            icon: "wallet",
            title: "Bolsos",
            sub: "Conta, poupan\xE7a, vale, previd\xEAncia e FGTS: s\xF3 dinheiro l\xEDquido entra no saldo projetado.",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "set-panel set-pockets",
            },
            BOLSOS.map((b) =>
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-pocket-row",
                  key: b.nm,
                },
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "set-pocket__ic",
                  },
                  /*#__PURE__*/ React.createElement(Icon, {
                    name: b.ic,
                    size: 16,
                  }),
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    style: {
                      flex: 1,
                      minWidth: 0,
                    },
                  },
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "set-pocket__nm",
                    },
                    b.nm,
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "set-pocket__sub",
                    },
                    b.sub,
                  ),
                ),
                b.liquid
                  ? null
                  : /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        className: "set-pocket__badge",
                      },
                      /*#__PURE__*/ React.createElement(
                        Badge,
                        {
                          tone: "neutral",
                        },
                        "Bloqueado",
                      ),
                    ),
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "set-pocket__amt",
                    style: {
                      color: b.liquid ? "var(--text)" : "var(--text-faint)",
                    },
                  },
                  b.amt,
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "set-row",
                style: {
                  borderTop: "1px solid var(--border)",
                },
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__main",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__t",
                  },
                  "Saldo l\xEDquido projetado",
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__d",
                  },
                  "Soma apenas os bolsos l\xEDquidos (conta, poupan\xE7a, VA). FGTS e previd\xEAncia ficam de fora.",
                ),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__ctl",
                },
                /*#__PURE__*/ React.createElement(
                  Button,
                  {
                    variant: "ghost",
                    size: "sm",
                    iconLeft: /*#__PURE__*/ React.createElement(Icon, {
                      name: "plus",
                      size: 14,
                    }),
                  },
                  "Adicionar bolso",
                ),
              ),
            ),
          ),
        );
      }

      /* ---- Seção: Lembrete diário ---- */
      function LembreteDiarioSection() {
        const [enabled, setEnabled] = React.useState(true);
        const [time, setTime] = React.useState("20:00");
        return /*#__PURE__*/ React.createElement(
          Section,
          {
            icon: "bell",
            title: "Lembrete di\xE1rio",
            sub: "Notifica\xE7\xE3o nativa no hor\xE1rio escolhido \u2014 dispara mesmo com o app fechado.",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "set-panel",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "set-row",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__main",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__t",
                  },
                  "Ativar lembrete",
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__d",
                  },
                  "Envia uma notifica\xE7\xE3o nativa no hor\xE1rio escolhido \u2014 agendada no sistema para disparar mesmo com o Neko fechado.",
                ),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__ctl",
                },
                /*#__PURE__*/ React.createElement(SegmentedControl, {
                  options: [
                    {
                      value: "on",
                      label: "Ligado",
                    },
                    {
                      value: "off",
                      label: "Desligado",
                    },
                  ],
                  value: enabled ? "on" : "off",
                  onChange: (val) => setEnabled(val === "on"),
                  size: "sm",
                  ariaLabel: "Ativar ou desativar lembrete di\xE1rio",
                }),
              ),
            ),
            enabled
              ? /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row",
                  },
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "set-row__main",
                    },
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__t",
                      },
                      "Hor\xE1rio",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__d",
                      },
                      "Hora local (24 h) para receber o aviso.",
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "set-row__ctl set-time-ctl",
                    },
                    /*#__PURE__*/ React.createElement("input", {
                      type: "time",
                      value: time,
                      onChange: (e) => setTime(e.currentTarget.value),
                      className: "set-time-input",
                      "aria-label": "Hor\xE1rio do lembrete di\xE1rio",
                    }),
                  ),
                )
              : null,
          ),
        );
      }

      /* ---- Seção: Teto do Diário ---- */
      function TetoDiarioSection() {
        const [raw, setRaw] = React.useState("50,00");
        const [saved, setSaved] = React.useState(false);
        function handleSave() {
          setSaved(true);
          setTimeout(() => setSaved(false), 2000);
        }
        return /*#__PURE__*/ React.createElement(
          Section,
          {
            icon: "sliders",
            title: "Teto do Di\xE1rio",
            sub: "Defina quanto pretende gastar por dia no vari\xE1vel. Deixe em branco para usar a m\xE9dia do m\xEAs anterior.",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "set-panel",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "set-row",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__main",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__t",
                  },
                  "Teto di\xE1rio (R$)",
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__d",
                  },
                  "Orienta a barra de progresso do check-in e o forecast dos dias futuros do m\xEAs. Em branco = usar a m\xE9dia do m\xEAs anterior automaticamente.",
                  saved
                    ? /*#__PURE__*/ React.createElement("strong", null, " Salvo.")
                    : null,
                ),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__ctl",
                },
                /*#__PURE__*/ React.createElement("input", {
                  type: "text",
                  inputMode: "decimal",
                  placeholder: "ex.: 50,00",
                  value: raw,
                  onChange: (e) => {
                    setRaw(e.currentTarget.value);
                    setSaved(false);
                  },
                  "aria-label": "Teto di\xE1rio em reais",
                  style: {
                    fontFamily: "var(--font-money)",
                    fontSize: "var(--fs-body)",
                    background: "var(--bg-subtle)",
                    border: "1px solid var(--border-input)",
                    borderRadius: "var(--radius-xs)",
                    color: "var(--text)",
                    padding: "4px 8px",
                    height: "var(--hit-min)",
                    width: "10ch",
                  },
                }),
                /*#__PURE__*/ React.createElement(
                  Button,
                  {
                    variant: "secondary",
                    size: "sm",
                    onClick: handleSave,
                  },
                  "Salvar",
                ),
              ),
            ),
          ),
        );
      }

      /* ---- Seção: Categorias do Diário ---- */
      const CAT_DEMO = [
        {
          name: "Alimentação",
          amount: "R$ 380,00",
          pct: 38,
        },
        {
          name: "Transporte",
          amount: "R$ 200,00",
          pct: 20,
        },
        {
          name: "Farmácia",
          amount: "R$ 150,00",
          pct: 15,
        },
        {
          name: "Lazer",
          amount: "R$ 150,00",
          pct: 15,
        },
        {
          name: "Outros",
          amount: "R$ 120,00",
          pct: 12,
        },
      ];
      function CategoriasDiarioSection() {
        return /*#__PURE__*/ React.createElement(
          Section,
          {
            icon: "layoutList",
            title: "Categorias do Di\xE1rio",
            sub: "Distribua o teto mensal do Di\xE1rio entre categorias (ex.: Alimenta\xE7\xE3o, Transporte). O teto por dia \xE9 a soma \xF7 dias do m\xEAs.",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "set-panel",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "set-row",
                style: {
                  borderBottom: "1px solid var(--border)",
                },
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__main",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__t",
                  },
                  "Teto mensal do Di\xE1rio (R$)",
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__d",
                  },
                  "Em branco = usar a soma das categorias abaixo como teto mensal.",
                ),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__ctl",
                },
                /*#__PURE__*/ React.createElement("input", {
                  type: "text",
                  inputMode: "decimal",
                  placeholder: "ex.: 1.250,00",
                  defaultValue: "1.000,00",
                  "aria-label": "Teto mensal do Di\xE1rio em reais",
                  style: {
                    fontFamily: "var(--font-money)",
                    fontSize: "var(--fs-body)",
                    background: "var(--bg-subtle)",
                    border: "1px solid var(--border-input)",
                    borderRadius: "var(--radius-xs)",
                    color: "var(--text)",
                    padding: "4px 8px",
                    height: "var(--hit-min)",
                    width: "12ch",
                  },
                }),
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "set-cats",
              },
              CAT_DEMO.map((c) =>
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-cat-row",
                    key: c.name,
                  },
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      style: {
                        flex: 1,
                        minWidth: 0,
                      },
                    },
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        style: {
                          display: "flex",
                          justifyContent: "space-between",
                          marginBottom: 4,
                        },
                      },
                      /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          className: "set-cat__name",
                        },
                        c.name,
                      ),
                      /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          className: "set-cat__amt",
                        },
                        c.amount,
                      ),
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-cat__bar",
                      },
                      /*#__PURE__*/ React.createElement("div", {
                        className: "set-cat__fill",
                        style: {
                          width: c.pct + "%",
                        },
                      }),
                    ),
                  ),
                ),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-cats__total",
                },
                /*#__PURE__*/ React.createElement(
                  "span",
                  null,
                  "Total mensal \xB7 30 dias no m\xEAs",
                ),
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "set-cats__total-amt",
                  },
                  "R$\xA01.000,00 \xA0\xB7\xA0 R$\xA033,33/dia",
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "set-row",
                style: {
                  borderTop: "1px solid var(--border)",
                },
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__ctl",
                  style: {
                    marginLeft: "auto",
                  },
                },
                /*#__PURE__*/ React.createElement(
                  Button,
                  {
                    variant: "ghost",
                    size: "sm",
                    iconLeft: /*#__PURE__*/ React.createElement(Icon, {
                      name: "plus",
                      size: 14,
                    }),
                  },
                  "Adicionar categoria",
                ),
                /*#__PURE__*/ React.createElement(
                  Button,
                  {
                    variant: "secondary",
                    size: "sm",
                  },
                  "Salvar categorias",
                ),
              ),
            ),
          ),
        );
      }

      /* ---- Seção: Seus dados ---- */
      function SeusDadosSection() {
        const [backupMsg, setBackupMsg] = React.useState(null);
        function doBackup() {
          setBackupMsg("Backup salvo.");
          setTimeout(() => setBackupMsg(null), 2500);
        }
        return /*#__PURE__*/ React.createElement(
          Section,
          {
            icon: "shield",
            title: "Seus dados",
            sub: "O Neko \xE9 local-first: n\xE3o existe conta Neko nem backend.",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "set-panel",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "set-row",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__main",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__t",
                  },
                  "Onde ficam os dados",
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__d",
                  },
                  "Banco SQLite em ",
                  /*#__PURE__*/ React.createElement(
                    "code",
                    null,
                    "~/Library/Application Support/Neko/neko.db",
                  ),
                  ", somente neste dispositivo.",
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "set-row",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__main",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__t",
                  },
                  "Backup do banco",
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__d",
                  },
                  "Salva uma c\xF3pia \xEDntegra (.db) onde voc\xEA escolher \u2014 leve para outro disco ou dispositivo.",
                  backupMsg
                    ? /*#__PURE__*/ React.createElement("strong", null, " ", backupMsg)
                    : null,
                ),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__ctl",
                },
                /*#__PURE__*/ React.createElement(
                  Button,
                  {
                    variant: "secondary",
                    size: "sm",
                    iconLeft: /*#__PURE__*/ React.createElement(Icon, {
                      name: "download",
                      size: 14,
                    }),
                    onClick: doBackup,
                  },
                  "Fazer backup",
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "set-row",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__main",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__t",
                  },
                  "Telemetria",
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__d",
                  },
                  "O Neko n\xE3o envia nenhum dado de uso. Suas finan\xE7as n\xE3o saem da sua m\xE1quina.",
                ),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__ctl",
                },
                /*#__PURE__*/ React.createElement(
                  Badge,
                  {
                    tone: "neutral",
                  },
                  "Desativada",
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "set-row",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__main",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__t",
                  },
                  "Vers\xE3o",
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "set-row__d",
                  },
                  /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      className: "set-meta",
                    },
                    /*#__PURE__*/ React.createElement(Icon, {
                      name: "check",
                      size: 13,
                      style: {
                        color: "var(--success-500)",
                      },
                    }),
                    "Neko Finance v0.1.0 \xB7 Tauri desktop",
                  ),
                ),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-row__ctl",
                },
                /*#__PURE__*/ React.createElement(
                  Button,
                  {
                    variant: "ghost",
                    size: "sm",
                  },
                  "Verificar atualiza\xE7\xF5es",
                ),
              ),
            ),
          ),
        );
      }

      /* ---- Componente raiz da tela ---- */
      function SettingsScreen() {
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "set",
          },
          /*#__PURE__*/ React.createElement(ConexaoSection, null),
          /*#__PURE__*/ React.createElement(ImportacaoLocalSection, null),
          /*#__PURE__*/ React.createElement(BolsosSection, null),
          /*#__PURE__*/ React.createElement(LembreteDiarioSection, null),
          /*#__PURE__*/ React.createElement(TetoDiarioSection, null),
          /*#__PURE__*/ React.createElement(CategoriasDiarioSection, null),
          /*#__PURE__*/ React.createElement(SeusDadosSection, null),
        );
      }
      window.SettingsScreen = SettingsScreen;
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "ui_kits/settings/SettingsScreen.jsx",
      error: String((e && e.message) || e),
    });
  }

  // ui_kits/shared/icons.jsx
  try {
    (() => {
      /* Neko Finance — Lucide-style icon set (ISC). 24×24, 1.75px stroke, round caps.
         Loaded as a plain helper (lowercase filename → not a DS component).
         Exposes window.Icon (React component) and window.ICON_PATHS. */
      (function () {
        const P = {
          dashboard:
            '<rect x="3" y="3" width="7" height="9" rx="1.5"/><rect x="14" y="3" width="7" height="5" rx="1.5"/><rect x="14" y="12" width="7" height="9" rx="1.5"/><rect x="3" y="16" width="7" height="5" rx="1.5"/>',
          receipt:
            '<path d="M5 21V4a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v17l-2.5-1.5L14 21l-2.5-1.5L9 21l-2.5-1.5L5 21Z"/><path d="M8 7h8M8 11h8M8 15h5"/>',
          sparkles:
            '<path d="M12 3l1.6 4.4L18 9l-4.4 1.6L12 15l-1.6-4.4L6 9l4.4-1.6L12 3Z"/><path d="M18 14l.8 2.2L21 17l-2.2.8L18 20l-.8-2.2L15 17l2.2-.8L18 14Z"/>',
          book: '<path d="M4 5a2 2 0 0 1 2-2h13v16H6a2 2 0 0 0-2 2V5Z"/><path d="M4 5v14M8 7h7M8 11h7"/>',
          settings:
            '<circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M4.2 4.2l2.1 2.1M17.7 17.7l2.1 2.1M2 12h3M19 12h3M4.2 19.8l2.1-2.1M17.7 6.3l2.1-2.1"/>',
          plus: '<path d="M12 5v14M5 12h14"/>',
          search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.2-3.2"/>',
          chevronDown: '<path d="m6 9 6 6 6-6"/>',
          chevronRight: '<path d="m9 6 6 6-6 6"/>',
          arrowUpRight: '<path d="M7 17 17 7M8 7h9v9"/>',
          arrowDownRight: '<path d="m7 7 10 10M17 8v9H8"/>',
          wallet:
            '<path d="M3 7a2 2 0 0 1 2-2h13a1 1 0 0 1 1 1v2"/><path d="M3 7v10a2 2 0 0 0 2 2h14a1 1 0 0 0 1-1v-3"/><path d="M21 11v4h-4a2 2 0 0 1 0-4h4Z"/>',
          creditCard:
            '<rect x="2" y="5" width="20" height="14" rx="2"/><path d="M2 10h20M6 15h4"/>',
          trendingUp: '<path d="m3 17 6-6 4 4 8-8"/><path d="M17 7h4v4"/>',
          alertTriangle:
            '<path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z"/><path d="M12 9v4M12 17h.01"/>',
          alertCircle: '<circle cx="12" cy="12" r="9"/><path d="M12 8v4M12 16h.01"/>',
          check: '<path d="m4 12 5 5L20 6"/>',
          checkCircle:
            '<circle cx="12" cy="12" r="9"/><path d="m8.5 12 2.5 2.5L16 9"/>',
          x: '<path d="M6 6 18 18M18 6 6 18"/>',
          table:
            '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18M3 15h18M9 3v18"/>',
          lock: '<rect x="4" y="10" width="16" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/>',
          shield:
            '<path d="M12 3 5 6v5c0 4.5 3 7.5 7 9 4-1.5 7-4.5 7-9V6l-7-3Z"/><path d="m9 12 2 2 4-4"/>',
          refresh: '<path d="M21 12a9 9 0 1 1-2.6-6.4"/><path d="M21 4v5h-5"/>',
          link: '<path d="M10 13a4 4 0 0 0 5.7.3l3-3a4 4 0 0 0-5.7-5.7L11.3 6"/><path d="M14 11a4 4 0 0 0-5.7-.3l-3 3a4 4 0 0 0 5.7 5.7L12.7 18"/>',
          dollar:
            '<path d="M12 2v20M17 6.5c0-2-2-3.5-5-3.5s-5 1.3-5 3.5 2 3 5 3.5 5 1.3 5 3.5-2 3.5-5 3.5-5-1.5-5-3.5"/>',
          filter: '<path d="M3 5h18l-7 8v6l-4 2v-8L3 5Z"/>',
          more: '<circle cx="5" cy="12" r="1.4"/><circle cx="12" cy="12" r="1.4"/><circle cx="19" cy="12" r="1.4"/>',
          pencil: '<path d="M16.5 4.5a2.1 2.1 0 0 1 3 3L8 19l-4 1 1-4 11.5-11.5Z"/>',
          sliders:
            '<path d="M4 6h10M18 6h2M4 12h2M10 12h10M4 18h7M15 18h5"/><circle cx="16" cy="6" r="2"/><circle cx="8" cy="12" r="2"/><circle cx="13" cy="18" r="2"/>',
          send: '<path d="M22 2 11 13M22 2l-7 20-4-9-9-4 20-7Z"/>',
          panelRight:
            '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/>',
          bell: '<path d="M18 8a6 6 0 0 0-12 0c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.7 21a2 2 0 0 1-3.4 0"/>',
          sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
          moon: '<path d="M21 12.8A9 9 0 1 1 11.2 3 7 7 0 0 0 21 12.8Z"/>',
          calendar:
            '<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M3 9h18M8 3v4M16 3v4"/>',
          piggy:
            '<path d="M19 11a5 5 0 0 0-5-5H9a6 6 0 0 0-6 6 4 4 0 0 0 2 3.5V19h3v-2h4v2h3v-3a5 5 0 0 0 2-4Z"/><path d="M16 10h.01"/>',
          key: '<circle cx="7.5" cy="15.5" r="3.5"/><path d="m10 13 8-8M15 5l3 3M14 9l2 2"/>',
          download: '<path d="M12 3v12M7 11l5 5 5-5M5 21h14"/>',
          calculator:
            '<rect x="4" y="2" width="16" height="20" rx="2"/><path d="M8 6h8M8 10h.01M12 10h.01M16 10h.01M8 14h.01M12 14h.01M16 14h.01M8 18h.01M12 18h.01M16 18h.01"/>',
          layoutList:
            '<rect x="3" y="4" width="6" height="6" rx="1"/><rect x="3" y="14" width="6" height="6" rx="1"/><path d="M13 5h8M13 9h5M13 15h8M13 19h5"/>',
          gitCompare:
            '<circle cx="6" cy="6" r="3"/><circle cx="18" cy="18" r="3"/><path d="M13 6h3a2 2 0 0 1 2 2v7"/><path d="M11 18H8a2 2 0 0 1-2-2V9"/>',
          calendarRange:
            '<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M3 10h18M16 3v4M8 3v4M7 14h.01M11 14h6M7 18h6M17 18h.01"/>',
          tags: '<path d="M11.2 2H4a2 2 0 0 0-2 2v7.2a2 2 0 0 0 .6 1.4l8.7 8.7a2.4 2.4 0 0 0 3.4 0l6.6-6.6a2.4 2.4 0 0 0 0-3.4l-8.7-8.7A2 2 0 0 0 11.2 2Z"/><circle cx="7.5" cy="7.5" r="1.4"/>',
          help: '<circle cx="12" cy="12" r="10"/><path d="M9.1 9a3 3 0 0 1 5.8 1c0 2-3 3-3 3"/><path d="M12 17h.01"/>',
        };
        function Icon({ name, size = 18, stroke = 1.75, className = "", style = {} }) {
          const d = P[name] || "";
          return React.createElement("svg", {
            width: size,
            height: size,
            viewBox: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            strokeWidth: stroke,
            strokeLinecap: "round",
            strokeLinejoin: "round",
            className,
            style,
            dangerouslySetInnerHTML: {
              __html: d,
            },
          });
        }
        window.Icon = Icon;
        window.ICON_PATHS = P;
      })();
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "ui_kits/shared/icons.jsx",
      error: String((e && e.message) || e),
    });
  }

  // ui_kits/tags/TagsScreen.jsx
  try {
    (() => {
      /* Neko Finance — Tags screen (new).
         "Rótulos do mês" — lista de tags com totais mensais, controle de exclusão dos cálculos,
         e painel de criação de nova tag com paleta de cores e emoji.
         PT-BR copy · R$ em mono tabular · zero dependências externas.
         Expõe window.TagsScreen. */

      const NS = window.NekoFinanceDesignSystem_9bd1cd;
      const { Button, MonthNav, Money, EmptyState } = NS;
      const Icon = window.Icon;

      /* ---- CSS (once-only) ---- */
      (function injectTagsCSS() {
        if (document.getElementById("tags-css")) return;
        const s = document.createElement("style");
        s.id = "tags-css";
        s.textContent = `
      /* Layout principal */
      .tags { max-width: 720px; margin: 0 auto; padding: var(--space-2); }

      /* Cabeçalho */
      .tags-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-4);
        margin-bottom: var(--space-6);
        flex-wrap: wrap;
      }
      .tags-header__lead { display: flex; flex-direction: column; gap: var(--space-1); }
      .tags-header__title {
        font-size: var(--fs-h2);
        font-weight: var(--fw-bold);
        letter-spacing: var(--ls-tight);
        margin: 0;
        color: var(--text-strong);
      }
      .tags-header__sub {
        color: var(--text-muted);
        font-size: var(--fs-sm);
        margin: 0;
        line-height: var(--lh-normal);
        max-width: 460px;
      }
      .tags-header__controls {
        display: flex;
        align-items: center;
        gap: var(--space-3);
        flex-shrink: 0;
      }

      /* Painel de nova tag */
      .tags-form {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
        padding: var(--space-6);
        margin-bottom: var(--space-6);
        background: var(--surface);
        border: var(--bw-hair) solid var(--border);
        border-radius: var(--radius-md);
        box-shadow: var(--shadow-1);
      }
      .tags-form__row {
        display: flex;
        gap: var(--space-3);
        flex-wrap: wrap;
      }
      .tags-form__input {
        flex: 1;
        min-width: 160px;
        padding: var(--space-3) var(--space-4);
        border-radius: var(--radius-sm);
        border: var(--bw-hair) solid var(--border-input);
        background: var(--surface-2);
        color: var(--text);
        font-size: var(--fs-body);
        font-family: var(--font-sans);
        outline: none;
      }
      .tags-form__input:focus {
        border-color: var(--border-focus);
        box-shadow: 0 0 0 2px var(--primary-quiet);
      }
      .tags-form__input--emoji {
        flex: 0 0 80px;
        min-width: 0;
      }
      .tags-form__palette-label {
        font-size: var(--fs-micro);
        font-weight: var(--fw-medium);
        color: var(--text-faint);
        letter-spacing: var(--ls-label);
        text-transform: uppercase;
        margin-bottom: var(--space-2);
      }
      .tags-form__palette {
        display: flex;
        gap: var(--space-2);
        align-items: center;
      }
      .tags-form__swatch {
        width: 24px;
        height: 24px;
        border-radius: 50%;
        cursor: pointer;
        border: 2px solid transparent;
        flex-shrink: 0;
        transition: var(--t-hover), transform var(--dur-fast) var(--ease-entrance);
      }
      .tags-form__swatch:focus-visible {
        outline: 2px solid var(--border-focus);
        outline-offset: 2px;
      }
      .tags-form__swatch--selected {
        border-color: var(--text);
        transform: scale(1.15);
      }
      @media (prefers-reduced-motion: reduce) {
        .tags-form__swatch { transition: none; transform: none !important; }
      }
      .tags-form__hint {
        font-size: var(--fs-micro);
        color: var(--text-faint);
        margin: 0;
        line-height: var(--lh-normal);
      }
      .tags-form__error {
        font-size: var(--fs-sm);
        color: var(--danger-400);
        margin: 0;
      }

      /* Lista de tags */
      .tags-list {
        list-style: none;
        margin: 0;
        padding: 0;
        display: flex;
        flex-direction: column;
        gap: 2px;
      }
      .tags-list__item {
        display: flex;
        align-items: center;
        gap: var(--space-3);
        padding: var(--space-4) var(--space-3);
        border-bottom: var(--bw-hair) solid var(--border);
        transition: background var(--dur-fast) var(--ease-standard);
      }
      .tags-list__item:hover {
        background: var(--surface-hover);
      }
      @media (prefers-reduced-motion: reduce) {
        .tags-list__item { transition: none; }
      }
      .tags-list__chip {
        width: 14px;
        height: 22px;
        border-radius: 3px 6px 6px 3px;
        flex-shrink: 0;
      }
      .tags-list__emoji {
        font-size: var(--fs-body);
        line-height: 1;
        flex-shrink: 0;
      }
      .tags-list__name {
        flex: 1;
        font-size: var(--fs-sm);
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }
      .tags-list__name--special {
        font-weight: var(--fw-bold);
        color: var(--text);
      }
      .tags-list__name--normal {
        font-weight: var(--fw-semibold);
        color: var(--text);
      }
      .tags-list__name--excluded {
        color: var(--text-muted);
      }
      .tags-list__total {
        flex-shrink: 0;
      }
      .tags-list__toggle {
        padding: var(--space-1) var(--space-2);
        border-radius: var(--radius-sm);
        border: var(--bw-hair) solid var(--border);
        font-size: var(--fs-xs);
        font-family: var(--font-sans);
        cursor: pointer;
        flex-shrink: 0;
        transition: var(--t-hover);
      }
      .tags-list__toggle--included {
        background: transparent;
        color: var(--text);
      }
      .tags-list__toggle--excluded {
        background: var(--surface-2);
        color: var(--text-muted);
      }
      .tags-list__toggle:focus-visible {
        outline: 2px solid var(--border-focus);
        outline-offset: 2px;
      }

      /* Rodapé sumário */
      .tags-summary {
        display: flex;
        align-items: baseline;
        justify-content: space-between;
        gap: var(--space-5);
        margin-top: var(--space-6);
        padding-top: var(--space-4);
        border-top: var(--bw-hair) solid var(--border-strong);
        flex-wrap: wrap;
      }
      .tags-summary__label {
        font-size: var(--fs-sm);
        color: var(--text-muted);
      }
      .tags-summary__total {
        font-family: var(--font-money);
        font-variant-numeric: tabular-nums;
        font-size: var(--fs-money-md);
        font-weight: var(--fw-semibold);
        color: var(--text);
      }
      .tags-summary__excluded-note {
        font-size: var(--fs-micro);
        color: var(--text-faint);
        margin: 0;
        margin-top: var(--space-1);
      }
      `;
        document.head.appendChild(s);
      })();

      /* ---- dados de demo representativos ---- */
      const PALETTE = [
        {
          value: "var(--cat-jade)",
          name: "Verde",
          hex: "#3fbf8f",
        },
        {
          value: "var(--cat-sky)",
          name: "Azul",
          hex: "#5fa8dc",
        },
        {
          value: "var(--cat-orchid)",
          name: "Orquídea",
          hex: "#c98bd4",
        },
        {
          value: "var(--cat-violet)",
          name: "Violeta",
          hex: "#8c8ae6",
        },
        {
          value: "var(--cat-teal)",
          name: "Turquesa",
          hex: "#5fc9c0",
        },
        {
          value: "var(--cat-amber)",
          name: "Âmbar",
          hex: "#ddb061",
        },
        {
          value: "var(--cat-coral)",
          name: "Coral",
          hex: "#e68a84",
        },
      ];
      const DEMO_TAGS = [
        {
          id: "1",
          name: "! Pagar",
          emoji: "",
          color: "var(--cat-coral)",
          is_special: true,
          exclude_from_totals: false,
          total_cents: -284500,
        },
        {
          id: "2",
          name: "! Fatura cartão",
          emoji: "",
          color: "var(--cat-violet)",
          is_special: true,
          exclude_from_totals: false,
          total_cents: -142000,
        },
        {
          id: "3",
          name: "Mercado",
          emoji: "🛒",
          color: "var(--cat-jade)",
          is_special: false,
          exclude_from_totals: false,
          total_cents: -73200,
        },
        {
          id: "4",
          name: "Alimentação fora",
          emoji: "🍽",
          color: "var(--cat-amber)",
          is_special: false,
          exclude_from_totals: false,
          total_cents: -38900,
        },
        {
          id: "5",
          name: "Transporte",
          emoji: "🚌",
          color: "var(--cat-sky)",
          is_special: false,
          exclude_from_totals: false,
          total_cents: -24100,
        },
        {
          id: "6",
          name: "Assinaturas",
          emoji: "📦",
          color: "var(--cat-teal)",
          is_special: false,
          exclude_from_totals: false,
          total_cents: -18700,
        },
        {
          id: "7",
          name: "Saúde",
          emoji: "🩺",
          color: "var(--cat-orchid)",
          is_special: false,
          exclude_from_totals: false,
          total_cents: -9800,
        },
        {
          id: "8",
          name: "Reembolso empresa",
          emoji: "",
          color: "var(--cat-jade)",
          is_special: false,
          exclude_from_totals: true,
          total_cents: -45000,
        },
      ];
      function fmtBRL(cents) {
        const abs = Math.abs(cents);
        const n = (abs / 100).toLocaleString("pt-BR", {
          minimumFractionDigits: 2,
          maximumFractionDigits: 2,
        });
        return "R$ " + n;
      }
      function shiftYm(ym, delta) {
        const [y, m] = ym.split("-").map(Number);
        const d = new Date(Date.UTC(y, m - 1 + delta, 1));
        return `${d.getUTCFullYear()}-${String(d.getUTCMonth() + 1).padStart(2, "0")}`;
      }
      const MONTH_NAMES = [
        "Janeiro",
        "Fevereiro",
        "Março",
        "Abril",
        "Maio",
        "Junho",
        "Julho",
        "Agosto",
        "Setembro",
        "Outubro",
        "Novembro",
        "Dezembro",
      ];
      function monthLabel(ym) {
        const [y, m] = ym.split("-").map(Number);
        return `${MONTH_NAMES[m - 1]} de ${y}`;
      }

      /* ---- Painel de nova tag ---- */
      function NewTagForm({ onCancel }) {
        const [name, setName] = React.useState("");
        const [emoji, setEmoji] = React.useState("");
        const [color, setColor] = React.useState(PALETTE[0].value);
        const [saving, setSaving] = React.useState(false);
        const swatchRefs = React.useRef([]);
        function handleSwatchKey(e, i) {
          const last = PALETTE.length - 1;
          let next = null;
          if (e.key === "ArrowRight" || e.key === "ArrowDown")
            next = i === last ? 0 : i + 1;
          else if (e.key === "ArrowLeft" || e.key === "ArrowUp")
            next = i === 0 ? last : i - 1;
          else if (e.key === "Home") next = 0;
          else if (e.key === "End") next = last;
          if (next === null) return;
          e.preventDefault();
          setColor(PALETTE[next].value);
          swatchRefs.current[next]?.focus();
        }
        function handleSubmit() {
          if (!name.trim() || saving) return;
          setSaving(true);
          // Simula criação (demo estático)
          setTimeout(() => {
            setSaving(false);
            onCancel();
          }, 600);
        }
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "tags-form",
            role: "region",
            "aria-label": "Nova tag",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "tags-form__row",
            },
            /*#__PURE__*/ React.createElement("input", {
              "aria-label": "Nome da tag",
              placeholder: "Nome (ex.: Mercado, ! Pagar)",
              value: name,
              onChange: (e) => setName(e.target.value),
              className: "tags-form__input",
              autoFocus: true,
            }),
            /*#__PURE__*/ React.createElement("input", {
              "aria-label": "Emoji da tag",
              placeholder: "Emoji",
              value: emoji,
              onChange: (e) => setEmoji(e.target.value),
              className: "tags-form__input tags-form__input--emoji",
              maxLength: 4,
            }),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            null,
            /*#__PURE__*/ React.createElement(
              "p",
              {
                className: "tags-form__palette-label",
                id: "palette-label",
              },
              "Cor da tag",
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "tags-form__palette",
                role: "radiogroup",
                "aria-labelledby": "palette-label",
              },
              PALETTE.map((c, i) =>
                /*#__PURE__*/ React.createElement("button", {
                  key: c.value,
                  ref: (el) => {
                    swatchRefs.current[i] = el;
                  },
                  type: "button",
                  role: "radio",
                  "aria-checked": color === c.value,
                  "aria-label": c.name,
                  tabIndex: color === c.value ? 0 : -1,
                  onClick: () => setColor(c.value),
                  onKeyDown: (e) => handleSwatchKey(e, i),
                  className: `tags-form__swatch${color === c.value ? " tags-form__swatch--selected" : ""}`,
                  style: {
                    background: c.value,
                  },
                }),
              ),
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "p",
            {
              className: "tags-form__hint",
            },
            'Tags que come\xE7am com "!" ficam no topo e s\xE3o marcadas como especiais. Use a tag "Reembolso empresa" como ignorada nos c\xE1lculos.',
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              style: {
                display: "flex",
                gap: "var(--space-3)",
                alignItems: "center",
              },
            },
            /*#__PURE__*/ React.createElement(
              Button,
              {
                onClick: handleSubmit,
                disabled: !name.trim() || saving,
                variant: "primary",
              },
              saving ? "Criando…" : "Criar tag",
            ),
            /*#__PURE__*/ React.createElement(
              Button,
              {
                variant: "ghost",
                onClick: onCancel,
              },
              "Cancelar",
            ),
          ),
        );
      }

      /* ---- Item de tag ---- */
      function TagItem({ tag }) {
        const [excluded, setExcluded] = React.useState(tag.exclude_from_totals);
        return /*#__PURE__*/ React.createElement(
          "li",
          {
            className: "tags-list__item",
          },
          /*#__PURE__*/ React.createElement("span", {
            "aria-hidden": "true",
            className: "tags-list__chip",
            style: {
              background: tag.color,
            },
          }),
          tag.emoji
            ? /*#__PURE__*/ React.createElement(
                "span",
                {
                  "aria-hidden": "true",
                  className: "tags-list__emoji",
                },
                tag.emoji,
              )
            : null,
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: [
                "tags-list__name",
                tag.is_special ? "tags-list__name--special" : "tags-list__name--normal",
                excluded ? "tags-list__name--excluded" : "",
              ]
                .filter(Boolean)
                .join(" "),
            },
            tag.name,
          ),
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: "tags-list__total",
            },
            /*#__PURE__*/ React.createElement(Money, {
              cents: tag.total_cents,
              size: "sm",
            }),
          ),
          /*#__PURE__*/ React.createElement(
            "button",
            {
              type: "button",
              role: "switch",
              "aria-checked": excluded,
              "aria-label": excluded
                ? `Incluir "${tag.name}" nos cálculos`
                : `Ignorar "${tag.name}" nos cálculos`,
              onClick: () => setExcluded((v) => !v),
              className: `tags-list__toggle${excluded ? " tags-list__toggle--excluded" : " tags-list__toggle--included"}`,
            },
            excluded ? "ignorado" : "incluído",
          ),
        );
      }

      /* ---- Tela completa ---- */
      function TagsScreen(props) {
        const todayYm = "2026-06";
        const [ym, setYm] = React.useState(todayYm);
        const [formOpen, setFormOpen] = React.useState(false);
        const totalCents = DEMO_TAGS.filter((t) => !t.exclude_from_totals).reduce(
          (s, t) => s + t.total_cents,
          0,
        );
        const excludedCount = DEMO_TAGS.filter((t) => t.exclude_from_totals).length;
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "tags",
          },
          /*#__PURE__*/ React.createElement(
            "header",
            {
              className: "tags-header",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "tags-header__lead",
              },
              /*#__PURE__*/ React.createElement(
                "h1",
                {
                  className: "tags-header__title",
                },
                "Tags",
              ),
              /*#__PURE__*/ React.createElement(
                "p",
                {
                  className: "tags-header__sub",
                },
                "Totais de ",
                monthLabel(ym),
                '. Tags s\xE3o diagn\xF3stico \u2014 para onde foi o dinheiro, n\xE3o or\xE7amento; "! Pagar" e similares ficam no topo.',
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "tags-header__controls",
              },
              /*#__PURE__*/ React.createElement(MonthNav, {
                label: monthLabel(ym),
                onPrev: () => setYm((v) => shiftYm(v, -1)),
                onNext: () => setYm((v) => shiftYm(v, 1)),
                onToday: () => setYm(todayYm),
                atToday: ym === todayYm,
                prevLabel: "M\xEAs anterior",
                nextLabel: "Pr\xF3ximo m\xEAs",
              }),
              /*#__PURE__*/ React.createElement(
                Button,
                {
                  onClick: () => setFormOpen((v) => !v),
                  variant: formOpen ? "ghost" : "primary",
                },
                formOpen ? "Cancelar" : "Nova tag",
              ),
            ),
          ),
          formOpen
            ? /*#__PURE__*/ React.createElement(NewTagForm, {
                onCancel: () => setFormOpen(false),
              })
            : null,
          /*#__PURE__*/ React.createElement(
            "ul",
            {
              className: "tags-list",
              "aria-label": "Tags do m\xEAs",
            },
            DEMO_TAGS.map((tag) =>
              /*#__PURE__*/ React.createElement(TagItem, {
                key: tag.id,
                tag: tag,
              }),
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "footer",
            {
              className: "tags-summary",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              null,
              /*#__PURE__*/ React.createElement(
                "p",
                {
                  className: "tags-summary__label",
                },
                "Total inclu\xEDdo nos c\xE1lculos",
              ),
              excludedCount > 0
                ? /*#__PURE__*/ React.createElement(
                    "p",
                    {
                      className: "tags-summary__excluded-note",
                    },
                    excludedCount,
                    " ",
                    excludedCount === 1 ? "tag ignorada" : "tags ignoradas",
                    " ",
                    "n\xE3o entram neste total.",
                  )
                : null,
            ),
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "tags-summary__total",
                "aria-label": `Total: ${fmtBRL(totalCents)}`,
                style: {
                  color: totalCents < 0 ? "var(--money-neg)" : "var(--money-pos)",
                },
              },
              totalCents < 0 ? "−" : "",
              fmtBRL(Math.abs(totalCents)),
            ),
          ),
        );
      }
      window.TagsScreen = TagsScreen;
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "ui_kits/tags/TagsScreen.jsx",
      error: String((e && e.message) || e),
    });
  }

  // ui_kits/totais/TotaisScreen.jsx
  try {
    (() => {
      /* Neko Finance — Totais screen (new).
         "Cálculos do mês" — performance, economizado, custo de vida, diário médio,
         movimentações e totais por titular.
         PT-BR copy · R$ em mono tabular · zero dependências externas.
         Expõe window.TotaisScreen. */

      const NS = window.NekoFinanceDesignSystem_9bd1cd;
      const { Money, MonthNav, InfoPopover, OwnerChip, Disclosure } = NS;
      const Icon = window.Icon;

      /* ---- CSS (once-only) ---- */
      (function injectTotaisCSS() {
        if (document.getElementById("totais-css")) return;
        const s = document.createElement("style");
        s.id = "totais-css";
        s.textContent = `
      /* Layout raiz */
      .tot {
        max-width: var(--content-max);
        margin: 0 auto;
        padding: var(--space-2);
        display: flex;
        flex-direction: column;
        gap: var(--space-0);
      }

      /* Cabeçalho */
      .tot-header {
        margin-bottom: var(--space-6);
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
      }
      .tot-header__top {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-4);
        flex-wrap: wrap;
      }
      .tot-header__h1 {
        font-size: var(--fs-h2);
        font-weight: var(--fw-bold);
        letter-spacing: var(--ls-tight);
        margin: 0;
        color: var(--text-strong);
      }
      .tot-header__desc {
        color: var(--text-muted);
        font-size: var(--fs-sm);
        margin: 0;
        line-height: var(--lh-normal);
      }

      /* Grelha de métricas */
      .tot-metrics {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
        gap: var(--space-5);
        margin-bottom: var(--space-8);
      }

      /* Card de métrica individual */
      .tot-card {
        background: var(--surface);
        border: var(--bw-hair) solid var(--border);
        border-radius: var(--radius-md);
        box-shadow: var(--elev-card);
        padding: var(--space-6);
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
      }
      .tot-card__label {
        font-size: var(--fs-label);
        font-weight: var(--fw-semibold);
        letter-spacing: var(--ls-label);
        text-transform: uppercase;
        color: var(--text-muted);
      }
      .tot-card__value {
        display: flex;
        align-items: baseline;
        gap: var(--space-3);
      }
      .tot-card__pct {
        font-family: var(--font-money);
        font-size: var(--fs-money-lg);
        font-weight: var(--fw-bold);
        font-variant-numeric: tabular-nums;
        color: var(--text-strong);
      }
      .tot-card__sublabel {
        font-size: var(--fs-sm);
        color: var(--text-faint);
        line-height: var(--lh-normal);
      }

      /* Chip de status (ponto + rótulo) */
      .tot-chip {
        display: inline-flex;
        align-items: center;
        gap: var(--space-2);
        align-self: flex-start;
        padding: 4px 11px 4px 9px;
        border-radius: var(--radius-pill);
        font-size: var(--fs-sm);
        font-weight: var(--fw-semibold);
      }
      .tot-chip__dot {
        width: 7px;
        height: 7px;
        border-radius: 50%;
        flex: none;
      }

      /* Cabeçalho de seção */
      .tot-section-head {
        font-size: var(--fs-label);
        font-weight: var(--fw-semibold);
        letter-spacing: var(--ls-label);
        text-transform: uppercase;
        color: var(--text-muted);
        margin: 0 0 var(--space-4);
      }

      /* Seção: Movimentações */
      .tot-movs {
        margin-bottom: var(--space-8);
      }
      .tot-movs__row {
        display: flex;
        gap: var(--space-8);
        flex-wrap: wrap;
      }
      .tot-mov {
        display: flex;
        flex-direction: column;
        gap: var(--space-1);
      }
      .tot-mov__label {
        font-size: var(--fs-sm);
        color: var(--text-muted);
      }
      .tot-mov__hint {
        font-size: var(--fs-micro);
        color: var(--text-faint);
      }

      /* Separador visual entre Saídas e Saída Total */
      .tot-mov--accent .tot-mov__label {
        color: var(--text);
        font-weight: var(--fw-semibold);
      }

      /* Barra de economizado (YTD) */
      .tot-ytd {
        margin-top: var(--space-4);
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
      }
      .tot-ytd__track {
        height: 5px;
        border-radius: var(--radius-pill);
        background: var(--bg-subtle);
        overflow: hidden;
      }
      .tot-ytd__fill {
        height: 100%;
        border-radius: var(--radius-pill);
        background: var(--chart-1);
        transition: width var(--dur-slow) var(--ease-entrance);
      }
      @media (prefers-reduced-motion: reduce) {
        .tot-ytd__fill { transition: none; }
      }
      .tot-ytd__label {
        font-size: var(--fs-micro);
        color: var(--text-faint);
        line-height: var(--lh-normal);
      }

      /* Seção: Por titular */
      .tot-owners {
        margin-bottom: var(--space-8);
      }
      .tot-owners__row {
        display: flex;
        gap: var(--space-8);
        flex-wrap: wrap;
      }
      .tot-owner {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
      }

      /* Disclosure nota metodológica */
      .tot-note {
        margin-top: var(--space-8);
      }
      `;
        document.head.appendChild(s);
      })();

      /* ---- helpers ---- */
      function fmtBRL(cents) {
        const abs = Math.abs(cents);
        const n = (abs / 100).toLocaleString("pt-BR", {
          minimumFractionDigits: 2,
          maximumFractionDigits: 2,
        });
        return (cents < 0 ? "−" : "") + "R$ " + n;
      }

      /* STATUS_TONE mapeia HealthLevel → tokens de cor */
      const STATUS_TONE = {
        strong: {
          dot: "var(--success-400)",
          fg: "var(--success-400)",
          bg: "var(--success-tint)",
        },
        steady: {
          dot: "var(--primary)",
          fg: "var(--primary-quiet-text)",
          bg: "var(--primary-quiet)",
        },
        watch: {
          dot: "var(--warning-400)",
          fg: "var(--warning-400)",
          bg: "var(--warning-tint)",
        },
        risk: {
          dot: "var(--danger-400)",
          fg: "var(--danger-400)",
          bg: "var(--danger-tint)",
        },
      };

      /* ---- StatusChip ---- */
      function StatusChip({ level, label }) {
        const t = STATUS_TONE[level] || STATUS_TONE.steady;
        return /*#__PURE__*/ React.createElement(
          "span",
          {
            className: "tot-chip",
            style: {
              background: t.bg,
              color: t.fg,
            },
          },
          /*#__PURE__*/ React.createElement("span", {
            "aria-hidden": "true",
            className: "tot-chip__dot",
            style: {
              background: t.dot,
            },
          }),
          label,
        );
      }

      /* ---- MetricCard ---- */
      function MetricCard({
        label,
        term,
        children,
        status,
        sublabel,
        ytdPct,
        ytdLabel,
      }) {
        return /*#__PURE__*/ React.createElement(
          "article",
          {
            className: "tot-card",
          },
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: "tot-card__label",
            },
            term
              ? /*#__PURE__*/ React.createElement(
                  InfoPopover,
                  {
                    term: term,
                  },
                  label,
                )
              : label,
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "tot-card__value",
            },
            children,
          ),
          status &&
            /*#__PURE__*/ React.createElement(StatusChip, {
              level: status.level,
              label: status.label,
            }),
          sublabel &&
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "tot-card__sublabel",
              },
              sublabel,
            ),
          ytdPct != null &&
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "tot-ytd",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "tot-ytd__track",
                  role: "progressbar",
                  "aria-valuenow": ytdPct,
                  "aria-valuemin": 0,
                  "aria-valuemax": 100,
                  "aria-label": `Economizado acumulado no ano: ${ytdPct}%`,
                },
                /*#__PURE__*/ React.createElement("div", {
                  className: "tot-ytd__fill",
                  style: {
                    width: `${ytdPct}%`,
                  },
                }),
              ),
              ytdLabel &&
                /*#__PURE__*/ React.createElement(
                  "p",
                  {
                    className: "tot-ytd__label",
                  },
                  ytdLabel,
                ),
            ),
        );
      }

      /* ---- MovTotal: item de movimentação ---- */
      function MovTotal({ label, cents, hint, sign = "none", accent }) {
        return /*#__PURE__*/ React.createElement(
          "span",
          {
            className: `tot-mov${accent ? " tot-mov--accent" : ""}`,
          },
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: "tot-mov__label",
            },
            label,
          ),
          /*#__PURE__*/ React.createElement(Money, {
            cents: cents,
            size: "md",
            sign: sign,
          }),
          hint &&
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "tot-mov__hint",
              },
              hint,
            ),
        );
      }

      /* ---- Dados de demonstração ---- */
      // Junho de 2026 — números realistas e representativos do método.
      const DEMO = {
        year: 2026,
        month: 6,
        monthLabel: "Junho",
        // Performance: Entradas − Saída Total
        performance_cents: 53200,
        // R$ 532,00 — sobra positiva

        // Economizado%: taxa de poupança do mês
        savings_rate_bps: 2240,
        // 22,40% — dentro do ideal (20–30%)

        // Custo de vida = Saídas + Diário
        cost_of_living_cents: 693800,
        // R$ 6.938,00

        // Entradas do mês
        income_cents: 747000,
        // R$ 7.470,00

        // Diário médio realizado
        real_daily_avg_cents: 14700,
        // R$ 147,00/dia

        // Movimentações individuais
        fixed_out_cents: 385000,
        // R$ 3.850,00 (saídas fixas + cartão)
        daily_out_cents: 308800,
        // R$ 3.088,00 (gasto variável diário)
        economia_cents: 0,
        // R$ 0,00 (neste mês não houve registro Economia)

        // YTD Economizado (anual)
        ytd_pct_raw: 18,
        // 18% acumulado no ano (abaixo de 20% — "Abaixo do ideal")
        ytd_pct: 18,
        // Por titular
        owners: [
          {
            id: "ana",
            name: "Ana",
            who: "personal",
            total_cents: 432600,
          },
          {
            id: "parceira",
            name: "Ana",
            who: "partner",
            total_cents: 261200,
          },
        ],
      };

      /* ---- Lógica de status (espelha totaisStatus.ts) ---- */
      function performanceStatus(cents) {
        return cents >= 0
          ? {
              level: "strong",
              label: "Sobrou dinheiro",
            }
          : {
              level: "risk",
              label: "Faltou dinheiro",
            };
      }
      function economizadoStatus(bps) {
        if (bps > 3000)
          return {
            level: "steady",
            label: "Acima do ideal",
          };
        if (bps >= 2000)
          return {
            level: "strong",
            label: "Dentro do ideal",
          };
        return {
          level: "watch",
          label: "Abaixo do ideal",
        };
      }
      function custoVidaStatus(cost, income) {
        return cost <= income
          ? {
              level: "steady",
              label: "Dentro da renda",
            }
          : {
              level: "watch",
              label: "Acima da renda",
            };
      }

      /* ---- Tela completa ---- */
      function TotaisScreen(props) {
        const m = DEMO;
        const pct = Math.round(m.savings_rate_bps / 100);
        const ytdPct = Math.min(m.ytd_pct_raw, 100);
        const ytdLabel =
          m.ytd_pct_raw > 100
            ? "no ano: >100% acumulado · meta 20–30% (média anual)"
            : `no ano: ${ytdPct}% acumulado · meta 20–30% (média anual)`;
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "tot",
          },
          /*#__PURE__*/ React.createElement(
            "header",
            {
              className: "tot-header",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "tot-header__top",
              },
              /*#__PURE__*/ React.createElement(
                "h1",
                {
                  className: "tot-header__h1",
                },
                "Totais",
              ),
              /*#__PURE__*/ React.createElement(MonthNav, {
                label: `${m.monthLabel} de ${m.year}`,
                onPrev: () => {},
                onNext: () => {},
                onToday: () => {},
                canPrev: true,
                canNext: false,
                atToday: true,
              }),
            ),
            /*#__PURE__*/ React.createElement(
              "p",
              {
                className: "tot-header__desc",
              },
              "C\xE1lculos do m\xEAs: performance, custo de vida, economizado e di\xE1rio m\xE9dio.",
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "section",
            {
              "aria-label": "C\xE1lculos do m\xEAs",
              className: "tot-metrics",
            },
            /*#__PURE__*/ React.createElement(
              MetricCard,
              {
                label: "Performance",
                term: "performance",
                status: performanceStatus(m.performance_cents),
              },
              /*#__PURE__*/ React.createElement(Money, {
                cents: m.performance_cents,
                size: "lg",
                sign: "auto",
              }),
            ),
            /*#__PURE__*/ React.createElement(
              MetricCard,
              {
                label: "Economizado",
                term: "economizado",
                status: economizadoStatus(m.savings_rate_bps),
                ytdPct: ytdPct,
                ytdLabel: ytdLabel,
              },
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "tot-card__pct",
                },
                pct,
                "%",
              ),
            ),
            /*#__PURE__*/ React.createElement(
              MetricCard,
              {
                label: "Custo de vida",
                term: "custo_de_vida",
                status: custoVidaStatus(m.cost_of_living_cents, m.income_cents),
                sublabel: "= Sa\xEDda Total (sa\xEDdas incl. cart\xE3o + di\xE1rio)",
              },
              /*#__PURE__*/ React.createElement(Money, {
                cents: m.cost_of_living_cents,
                size: "lg",
              }),
            ),
            /*#__PURE__*/ React.createElement(
              MetricCard,
              {
                label: "Di\xE1rio m\xE9dio",
                term: "diario_medio",
                sublabel: "m\xE9dia realizada por dia at\xE9 hoje",
              },
              /*#__PURE__*/ React.createElement(Money, {
                cents: m.real_daily_avg_cents,
                size: "lg",
              }),
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "section",
            {
              "aria-label": "Movimenta\xE7\xF5es do m\xEAs",
              className: "tot-movs",
            },
            /*#__PURE__*/ React.createElement(
              "h2",
              {
                className: "tot-section-head",
              },
              "Movimenta\xE7\xF5es do m\xEAs",
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "tot-movs__row",
              },
              /*#__PURE__*/ React.createElement(MovTotal, {
                label: "Entradas",
                cents: m.income_cents,
                sign: "auto",
              }),
              /*#__PURE__*/ React.createElement(MovTotal, {
                label: "Sa\xEDdas",
                cents: m.fixed_out_cents,
                hint: "fixas (cart\xE3o entra aqui)",
              }),
              /*#__PURE__*/ React.createElement(MovTotal, {
                label: "Di\xE1rio",
                cents: m.daily_out_cents,
                hint: "gasto vari\xE1vel",
              }),
              /*#__PURE__*/ React.createElement(MovTotal, {
                label: "Economia",
                cents: m.economia_cents,
                hint: "guardado no m\xEAs",
              }),
              /*#__PURE__*/ React.createElement(MovTotal, {
                label: "Sa\xEDda Total",
                cents: m.cost_of_living_cents,
                hint: "sa\xEDdas (incl. cart\xE3o) + di\xE1rio = custo de vida",
                accent: true,
              }),
            ),
          ),
          m.owners.length >= 2 &&
            /*#__PURE__*/ React.createElement(
              "section",
              {
                "aria-label": "Por titular",
                className: "tot-owners",
              },
              /*#__PURE__*/ React.createElement(
                "h2",
                {
                  className: "tot-section-head",
                },
                "Por titular",
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "tot-owners__row",
                },
                m.owners.map((o) =>
                  /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      key: o.id,
                      className: "tot-owner",
                    },
                    /*#__PURE__*/ React.createElement(OwnerChip, {
                      name: o.name,
                      who: o.who,
                      avatar: true,
                    }),
                    /*#__PURE__*/ React.createElement(Money, {
                      cents: o.total_cents,
                      size: "md",
                    }),
                  ),
                ),
              ),
            ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "tot-note",
            },
            /*#__PURE__*/ React.createElement(
              Disclosure,
              {
                title: "Como o Neko calcula estes totais",
              },
              /*#__PURE__*/ React.createElement(
                "p",
                {
                  style: {
                    fontSize: "var(--fs-sm)",
                    color: "var(--text-muted)",
                    margin: 0,
                    lineHeight: "var(--lh-normal)",
                  },
                },
                /*#__PURE__*/ React.createElement("strong", null, "Performance"),
                " = Entradas \u2212 Sa\xEDda Total. Positivo significa que o m\xEAs ficou dentro da renda. ",
                /*#__PURE__*/ React.createElement("strong", null, "Economizado%"),
                " = o que foi registrado como Economia \xF7 Entradas (meta 20\u201330% em m\xE9dia anual).",
                " ",
                /*#__PURE__*/ React.createElement("strong", null, "Custo de vida"),
                " = Sa\xEDdas fixas + Di\xE1rio \u2014 inclui cart\xE3o de cr\xE9dito no vencimento. O ",
                /*#__PURE__*/ React.createElement("strong", null, "Di\xE1rio m\xE9dio"),
                " \xE9 a m\xE9dia realizada por dia at\xE9 hoje, n\xE3o uma meta. Estes c\xE1lculos espelham diretamente as colunas da planilha do m\xE9todo.",
              ),
            ),
          ),
        );
      }
      window.TotaisScreen = TotaisScreen;
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "ui_kits/totais/TotaisScreen.jsx",
      error: String((e && e.message) || e),
    });
  }

  // ui_kits/transactions/TransactionsScreen.jsx
  try {
    (() => {
      /* Neko Finance — Livro-razão (Lançamentos). Tabela de histórico com filtros, tipos de
         movimento (MovBadge), proveniência (ProvBadge), tags, titulares (OwnerChip) e painel
         de ações inline. Expõe window.TransactionsScreen. */
      const NS = window.NekoFinanceDesignSystem_9bd1cd;
      const {
        Badge,
        Button,
        SegmentedControl,
        OwnerChip,
        MovBadge,
        ProvBadge,
        Money,
        EmptyState,
      } = NS;
      const Icon = window.Icon;
      (function injectCSS() {
        if (document.getElementById("txs-css")) return;
        const s = document.createElement("style");
        s.id = "txs-css";
        s.textContent = `
      .dash{display:flex;flex-direction:column;gap:14px;}
      .dash-card{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);box-shadow:var(--shadow-1);}
      .dash-card__body{padding:8px 16px 16px;}

      /* ---- toolbar ---- */
      .txs-tools{display:flex;align-items:center;gap:10px;flex-wrap:wrap;}
      .txs-tools__sp{flex:1;}

      /* ---- ledger table ---- */
      .txn-table{width:100%;border-collapse:collapse;font-size:var(--fs-sm);font-family:var(--font-sans);}
      .txn-table thead tr{border-bottom:1px solid var(--border);background:var(--bg-subtle);}
      .txn-table th{padding:8px 12px;text-align:left;font-size:10.5px;font-weight:700;letter-spacing:.06em;
        text-transform:uppercase;color:var(--text-faint);white-space:nowrap;}
      .txn-table th:last-child{width:32px;padding-right:8px;}
      .txn-table td{padding:9px 12px;vertical-align:middle;border-bottom:1px solid var(--border);color:var(--text);}
      .txn-table tr:last-child td{border-bottom:none;}
      .txn-table tr.projection td{opacity:.65;}
      .txn-table tr:hover td{background:var(--surface-hover);}
      .txn-table td:nth-child(5){text-align:right;font-family:var(--font-money);font-variant-numeric:tabular-nums;
        white-space:nowrap;}
      .txn-table td:last-child{text-align:right;padding-right:8px;}

      /* month separator */
      .txn-month-sep th{padding:10px 12px 6px;font-size:11px;font-weight:700;letter-spacing:.05em;
        text-transform:uppercase;color:var(--text-faint);border-bottom:1px solid var(--border);
        background:var(--bg);}

      /* expandable sub-rows */
      .txn-tag-editor td{padding:6px 12px 10px;background:var(--bg-subtle);border-bottom:1px solid var(--border);}

      /* tag chip */
      .txn-chip{display:inline-flex;align-items:center;gap:4px;height:18px;padding:0 7px 0 5px;
        border-radius:var(--radius-pill);background:var(--surface-2);border:var(--bw-hair) solid currentColor;
        font-size:var(--fs-micro);font-weight:var(--fw-medium);margin-left:5px;vertical-align:middle;}
      .txn-tag-dot{width:6px;height:6px;border-radius:50%;flex:none;}

      /* inline action buttons */
      .txn-tag-btn{border:none;background:none;cursor:pointer;color:var(--text-faint);padding:2px 4px;
        border-radius:var(--radius-xs);line-height:1;vertical-align:middle;transition:var(--t-hover);}
      .txn-tag-btn:hover{color:var(--text);background:var(--surface-hover);}

      /* method text */
      .txn-method{color:var(--text-muted);font-size:var(--fs-sm);}

      /* tag picker */
      .txn-tag-picker{display:flex;flex-wrap:wrap;gap:6px;}
      .txn-tag-opt{display:inline-flex;align-items:center;gap:5px;padding:4px 9px;border-radius:var(--radius-sm);
        border:var(--bw-hair) solid var(--border);background:var(--surface-elevated);
        font-size:var(--fs-sm);font-weight:var(--fw-medium);color:var(--text-muted);
        cursor:pointer;transition:var(--t-hover);}
      .txn-tag-opt.is-on{border-color:var(--primary);background:var(--primary-quiet);color:var(--text-strong);}
      .txn-tag-opt:hover:not(.is-on){border-color:var(--border-strong);color:var(--text);}

      /* inline error */
      .txs-inline-error{margin:0 0 6px;font-size:var(--fs-sm);color:var(--danger-400);}

      /* action panel */
      .txn-imported-notice{margin:0 0 6px;font-size:var(--fs-micro);color:var(--text-faint);}

      /* due date chip */
      .txn-due-chip{display:inline-flex;align-items:center;gap:5px;height:20px;margin-left:6px;padding:0 8px;
        border-radius:var(--radius-pill);background:var(--surface-2);border:var(--bw-hair) solid var(--border);
        font-size:var(--fs-micro);font-weight:var(--fw-medium);color:var(--text-muted);
        white-space:nowrap;vertical-align:middle;}

      /* installment badge */
      .txn-inst-badge{display:inline-flex;align-items:center;height:20px;margin-left:6px;padding:0 8px;
        border-radius:var(--radius-pill);background:var(--surface-2);border:var(--bw-hair) solid var(--border);
        font-size:var(--fs-micro);font-weight:var(--fw-medium);color:var(--text-muted);
        white-space:nowrap;vertical-align:middle;}

      /* line items list */
      .txn-items-list{display:flex;flex-direction:column;gap:var(--space-1);margin:0;
        padding-left:var(--space-6);list-style:none;}
      .txn-item-row{display:flex;gap:var(--space-3);align-items:baseline;font-size:var(--fs-sm);
        color:var(--text-muted);}

      /* generic / italic description */
      .txn-desc-faint{color:var(--text-faint);font-style:italic;}

      @media (prefers-reduced-motion:reduce){
        .txn-tag-btn,.txn-tag-opt{transition:none;}
      }
      `;
        document.head.appendChild(s);
      })();

      /* ---- Demo data ---- */
      const DEMO_TRANSACTIONS = [
        /* Junho 2026 */
        {
          id: "t-001",
          date: "2026-06-20",
          type: "expense",
          is_fixed: false,
          payment_method: "credit",
          provenance: "manual",
          description: "Delivery — Jantar",
          amount: -4750,
          owners: [],
          tags: [
            {
              id: "tag-1",
              name: "Alimentação",
              color: "#e0a33e",
              emoji: "",
            },
          ],
          line_items: [],
          due_date: null,
          installment_index: null,
          installment_total: null,
          is_projection: false,
        },
        {
          id: "t-002",
          date: "2026-06-18",
          type: "expense",
          is_fixed: true,
          payment_method: "debit",
          provenance: "importado",
          description: "Aluguel — Junho",
          amount: -180000,
          owners: [],
          tags: [],
          line_items: [],
          due_date: null,
          installment_index: null,
          installment_total: null,
          is_projection: false,
        },
        {
          id: "t-003",
          date: "2026-06-17",
          type: "income",
          is_fixed: false,
          payment_method: null,
          provenance: "importado",
          description: "Salário — Empresa XYZ",
          amount: 620000,
          owners: [],
          tags: [],
          line_items: [],
          due_date: null,
          installment_index: null,
          installment_total: null,
          is_projection: false,
        },
        {
          id: "t-004",
          date: "2026-06-15",
          type: "expense",
          is_fixed: false,
          payment_method: "pix",
          provenance: "manual",
          description: "Marketplace — Fone de ouvido",
          amount: -25990,
          owners: [],
          tags: [
            {
              id: "tag-2",
              name: "Eletrônicos",
              color: "#5fa8dc",
              emoji: "",
            },
          ],
          line_items: [],
          due_date: null,
          installment_index: 2,
          installment_total: 3,
          is_projection: false,
        },
        {
          id: "t-005",
          date: "2026-06-12",
          type: "expense",
          is_fixed: false,
          payment_method: "credit",
          provenance: "importado",
          description: "Supermercado Central",
          amount: -38400,
          owners: [],
          tags: [],
          line_items: [
            {
              id: "li-1",
              description: "Hortifruti",
              amount_cents: 8900,
            },
            {
              id: "li-2",
              description: "Limpeza",
              amount_cents: 12300,
            },
            {
              id: "li-3",
              description: "Laticínios",
              amount_cents: 17200,
            },
          ],
          due_date: null,
          installment_index: null,
          installment_total: null,
          is_projection: false,
        },
        {
          id: "t-006",
          date: "2026-06-10",
          type: "transfer",
          is_fixed: false,
          payment_method: "pix",
          provenance: "manual",
          description: "Poupança — aporte mensal",
          amount: -50000,
          owners: [],
          tags: [],
          line_items: [],
          due_date: null,
          installment_index: null,
          installment_total: null,
          is_projection: false,
        },
        {
          id: "t-007",
          date: "2026-06-05",
          type: "expense",
          is_fixed: true,
          payment_method: "debit",
          provenance: "importado",
          description: "Streaming",
          amount: -5490,
          owners: [],
          tags: [],
          line_items: [],
          due_date: null,
          installment_index: null,
          installment_total: null,
          is_projection: false,
        } /* Maio 2026 */,
        {
          id: "t-008",
          date: "2026-05-30",
          type: "expense",
          is_fixed: false,
          payment_method: "pix",
          provenance: "importado",
          description: "Dentista",
          amount: -35000,
          owners: [],
          tags: [],
          line_items: [],
          due_date: null,
          installment_index: null,
          installment_total: null,
          is_projection: false,
        },
        {
          id: "t-009",
          date: "2026-05-17",
          type: "income",
          is_fixed: false,
          payment_method: null,
          provenance: "importado",
          description: "Salário — Empresa XYZ",
          amount: 620000,
          owners: [],
          tags: [],
          line_items: [],
          due_date: null,
          installment_index: null,
          installment_total: null,
          is_projection: false,
        },
        {
          id: "t-010",
          date: "2026-05-10",
          type: "expense",
          is_fixed: false,
          payment_method: "credit",
          provenance: "importado",
          description: "Posto Ipiranga",
          amount: -9200,
          owners: [],
          tags: [],
          line_items: [],
          due_date: null,
          installment_index: null,
          installment_total: null,
          is_projection: false,
        } /* projetados */,
        {
          id: "t-011",
          date: "2026-07-01",
          type: "expense",
          is_fixed: true,
          payment_method: "debit",
          provenance: "projetado",
          description: "Aluguel — Julho",
          amount: -180000,
          owners: [],
          tags: [],
          line_items: [],
          due_date: "2026-07-05",
          installment_index: null,
          installment_total: null,
          is_projection: true,
        },
      ];
      const METHOD_LABELS = {
        debit: "Débito",
        credit: "Crédito",
        pix: "PIX",
        cash: "Dinheiro",
      };
      function methodLabel(t) {
        if (t.payment_method)
          return METHOD_LABELS[t.payment_method] ?? t.payment_method;
        return t.type === "income" ? "Entrada" : "—";
      }
      function movKind(t) {
        if (t.type === "income") return "entrada";
        if (t.type === "transfer") return "economia";
        if (t.is_fixed) return "saida";
        if (t.payment_method === "credit") return "cartao";
        return "diario";
      }
      function fmtDate(iso) {
        const [y, m, d] = iso.split("-");
        return `${d}/${m}/${y}`;
      }
      function monthLabel(ym) {
        const months = [
          "Janeiro",
          "Fevereiro",
          "Março",
          "Abril",
          "Maio",
          "Junho",
          "Julho",
          "Agosto",
          "Setembro",
          "Outubro",
          "Novembro",
          "Dezembro",
        ];
        const [y, m] = ym.split("-");
        return `${months[parseInt(m, 10) - 1]} de ${y}`;
      }
      function fmtBRL(cents) {
        const abs = Math.abs(cents);
        const reais = Math.floor(abs / 100);
        const centavos = String(abs % 100).padStart(2, "0");
        const formatted = reais.toLocaleString("pt-BR");
        const prefix = cents < 0 ? "− R$ " : "R$ ";
        return `${prefix}${formatted},${centavos}`;
      }

      /* ---- Tag chip (somente leitura) ---- */
      function TagChip({ tag }) {
        return /*#__PURE__*/ React.createElement(
          "span",
          {
            className: "txn-chip",
            style: {
              borderColor: tag.color,
              color: "var(--text)",
            },
          },
          /*#__PURE__*/ React.createElement("span", {
            "aria-hidden": "true",
            className: "txn-tag-dot",
            style: {
              background: tag.color,
            },
          }),
          tag.emoji ? `${tag.emoji} ` : "",
          tag.name,
        );
      }

      /* ---- Painel de itens itemizados ---- */
      function LineItemsPanel({ t }) {
        if (!t.line_items || t.line_items.length === 0) return null;
        const sign = t.type === "income" ? 1 : -1;
        return /*#__PURE__*/ React.createElement(
          "tr",
          {
            className: "txn-tag-editor",
          },
          /*#__PURE__*/ React.createElement(
            "td",
            {
              colSpan: 6,
            },
            /*#__PURE__*/ React.createElement(
              "ul",
              {
                className: "txn-items-list",
                "aria-label": `Itens de ${t.description || "lançamento"}`,
              },
              t.line_items.map((li) =>
                /*#__PURE__*/ React.createElement(
                  "li",
                  {
                    key: li.id,
                    className: "txn-item-row",
                  },
                  /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      style: {
                        fontFamily: "var(--font-money)",
                        fontVariantNumeric: "tabular-nums",
                        fontSize: "var(--fs-sm)",
                        color: sign < 0 ? "var(--money-neg)" : "var(--money-pos)",
                        whiteSpace: "nowrap",
                      },
                    },
                    fmtBRL(sign * Math.abs(li.amount_cents)),
                  ),
                  /*#__PURE__*/ React.createElement("span", null, li.description),
                ),
              ),
            ),
          ),
        );
      }

      /* ---- Painel de ações (Editar / Apagar) ---- */
      function ActionPanel({ t, onClose }) {
        const isImported = t.provenance === "importado";
        const isRecurring = t.id.includes(":");
        return /*#__PURE__*/ React.createElement(
          "tr",
          {
            className: "txn-tag-editor",
          },
          /*#__PURE__*/ React.createElement(
            "td",
            {
              colSpan: 6,
            },
            isImported &&
              /*#__PURE__*/ React.createElement(
                "p",
                {
                  className: "txn-imported-notice",
                },
                "Linha importada da planilha \u2014 edi\xE7\xF5es ficam no app; um re-import pode sobrescrever o valor se a planilha mudou. Apagar aqui n\xE3o apaga da planilha; o pr\xF3ximo import restaura a linha.",
              ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                style: {
                  display: "flex",
                  gap: "var(--space-3)",
                  flexWrap: "wrap",
                  alignItems: "center",
                },
              },
              /*#__PURE__*/ React.createElement(
                Button,
                {
                  size: "sm",
                  variant: "ghost",
                  onClick: onClose,
                },
                "Editar",
              ),
              isRecurring
                ? /*#__PURE__*/ React.createElement(
                    Button,
                    {
                      size: "sm",
                      variant: "ghost",
                      onClick: onClose,
                    },
                    "Apagar da s\xE9rie",
                  )
                : /*#__PURE__*/ React.createElement(
                    Button,
                    {
                      size: "sm",
                      variant: "ghost",
                      onClick: onClose,
                    },
                    "Apagar",
                  ),
            ),
          ),
        );
      }

      /* ---- Painel de editor de tags ---- */
      function TagEditorPanel({ t, onClose }) {
        const allTags = [
          {
            id: "tag-1",
            name: "Alimentação",
            color: "#e0a33e",
            emoji: "",
          },
          {
            id: "tag-2",
            name: "Eletrônicos",
            color: "#5fa8dc",
            emoji: "",
          },
          {
            id: "tag-3",
            name: "Saúde",
            color: "#4fd39a",
            emoji: "",
          },
          {
            id: "tag-4",
            name: "Lazer",
            color: "#c98bd4",
            emoji: "",
          },
          {
            id: "tag-5",
            name: "Moradia",
            color: "#e0625b",
            emoji: "",
          },
        ];
        const activeIds = new Set(t.tags.map((x) => x.id));
        return /*#__PURE__*/ React.createElement(
          "tr",
          {
            className: "txn-tag-editor",
          },
          /*#__PURE__*/ React.createElement(
            "td",
            {
              colSpan: 6,
            },
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "txn-tag-picker",
              },
              allTags.map((tag) => {
                const on = activeIds.has(tag.id);
                return /*#__PURE__*/ React.createElement(
                  "button",
                  {
                    key: tag.id,
                    type: "button",
                    "aria-pressed": on,
                    className: `txn-tag-opt${on ? " is-on" : ""}`,
                  },
                  /*#__PURE__*/ React.createElement("span", {
                    "aria-hidden": "true",
                    className: "txn-tag-dot",
                    style: {
                      background: tag.color,
                    },
                  }),
                  tag.name,
                );
              }),
            ),
          ),
        );
      }

      /* ---- Linha do ledger ---- */
      function LedgerRow({
        t,
        itemsOpen,
        actionOpen,
        tagOpen,
        onToggleItems,
        onToggleAction,
        onToggleTag,
      }) {
        const hasItems = t.line_items && t.line_items.length > 0;
        const isGeneric =
          t.description &&
          /^(Entrada|Saída|Diário) \d{4}-\d{2}-\d{2}$/.test(t.description);
        return /*#__PURE__*/ React.createElement(
          "tr",
          {
            className: t.is_projection ? "projection" : "",
          },
          /*#__PURE__*/ React.createElement(
            "td",
            {
              style: {
                whiteSpace: "nowrap",
                color: "var(--text-muted)",
              },
            },
            fmtDate(t.date),
          ),
          /*#__PURE__*/ React.createElement(
            "td",
            null,
            /*#__PURE__*/ React.createElement(MovBadge, {
              kind: movKind(t),
              showLabel: true,
              size: 16,
            }),
          ),
          /*#__PURE__*/ React.createElement(
            "td",
            null,
            hasItems &&
              /*#__PURE__*/ React.createElement(
                "button",
                {
                  type: "button",
                  className: "txn-tag-btn",
                  "aria-label": `${itemsOpen ? "Fechar" : "Ver"} itens de ${t.description || "lançamento"}`,
                  "aria-expanded": itemsOpen,
                  onClick: onToggleItems,
                },
                itemsOpen
                  ? /*#__PURE__*/ React.createElement(Icon, {
                      name: "chevronDown",
                      size: 13,
                    })
                  : /*#__PURE__*/ React.createElement(Icon, {
                      name: "chevronRight",
                      size: 13,
                    }),
              ),
            " ",
            t.description
              ? /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: isGeneric ? "txn-desc-faint" : "",
                    title: isGeneric
                      ? "Sem nota na célula — reimporte via Google Sheets"
                      : undefined,
                  },
                  t.description,
                )
              : "—",
            " ",
            /*#__PURE__*/ React.createElement(ProvBadge, {
              provenance: t.provenance,
            }),
            t.due_date &&
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "txn-due-chip",
                  "aria-label": `Vencimento: ${fmtDate(t.due_date)}`,
                },
                /*#__PURE__*/ React.createElement(Icon, {
                  name: "calendar",
                  size: 11,
                }),
                fmtDate(t.due_date),
              ),
            t.installment_index != null &&
              t.installment_total != null &&
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "txn-inst-badge",
                  "aria-label": `Parcela ${t.installment_index} de ${t.installment_total}`,
                },
                t.installment_index,
                "/",
                t.installment_total,
                " parcelas",
              ),
            t.owners &&
              t.owners.length >= 2 &&
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  style: {
                    display: "inline-flex",
                    gap: 4,
                    marginLeft: 6,
                    verticalAlign: "middle",
                  },
                },
                t.owners.map((name) =>
                  /*#__PURE__*/ React.createElement(OwnerChip, {
                    key: name,
                    name: name,
                  }),
                ),
              ),
            t.tags &&
              t.tags.map((tag) =>
                /*#__PURE__*/ React.createElement(TagChip, {
                  key: tag.id,
                  tag: tag,
                }),
              ),
            /*#__PURE__*/ React.createElement(
              "button",
              {
                type: "button",
                className: "txn-tag-btn",
                "aria-label": `Editar tags de ${t.description || "lançamento"}`,
                "aria-expanded": tagOpen,
                onClick: onToggleTag,
                style: {
                  marginLeft: 4,
                },
              },
              /*#__PURE__*/ React.createElement(Icon, {
                name: "tags",
                size: 13,
              }),
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "td",
            null,
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "txn-method",
              },
              methodLabel(t),
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "td",
            {
              style: {
                textAlign: "right",
              },
            },
            /*#__PURE__*/ React.createElement(
              "span",
              {
                style: {
                  fontFamily: "var(--font-money)",
                  fontVariantNumeric: "tabular-nums",
                  fontSize: "var(--fs-sm)",
                  fontWeight: "var(--fw-semibold)",
                  color:
                    t.type === "income"
                      ? "var(--money-pos)"
                      : t.is_projection
                        ? "var(--text-faint)"
                        : "var(--money-neg)",
                  whiteSpace: "nowrap",
                },
              },
              fmtBRL(t.type === "income" ? Math.abs(t.amount) : -Math.abs(t.amount)),
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "td",
            {
              style: {
                width: 32,
                textAlign: "right",
                paddingRight: 8,
              },
            },
            /*#__PURE__*/ React.createElement(
              "button",
              {
                type: "button",
                className: "txn-tag-btn",
                "aria-label": `Ações para ${t.description || "lançamento"}`,
                "aria-expanded": actionOpen,
                onClick: onToggleAction,
              },
              /*#__PURE__*/ React.createElement(Icon, {
                name: "more",
                size: 13,
              }),
            ),
          ),
        );
      }

      /* ---- Tabela do Livro-razão ---- */
      function LedgerTable({ rows }) {
        const [itemsId, setItemsId] = React.useState(null);
        const [actionId, setActionId] = React.useState(null);
        const [tagId, setTagId] = React.useState(null);
        function toggleItems(id) {
          setItemsId((prev) => (prev === id ? null : id));
        }
        function toggleAction(id) {
          setActionId((prev) => (prev === id ? null : id));
        }
        function toggleTag(id) {
          setTagId((prev) => (prev === id ? null : id));
        }
        return /*#__PURE__*/ React.createElement(
          "table",
          {
            className: "txn-table",
          },
          /*#__PURE__*/ React.createElement(
            "thead",
            null,
            /*#__PURE__*/ React.createElement(
              "tr",
              null,
              /*#__PURE__*/ React.createElement(
                "th",
                {
                  scope: "col",
                },
                "Data",
              ),
              /*#__PURE__*/ React.createElement(
                "th",
                {
                  scope: "col",
                },
                "Tipo",
              ),
              /*#__PURE__*/ React.createElement(
                "th",
                {
                  scope: "col",
                },
                "Descri\xE7\xE3o",
              ),
              /*#__PURE__*/ React.createElement(
                "th",
                {
                  scope: "col",
                },
                "M\xE9todo",
              ),
              /*#__PURE__*/ React.createElement(
                "th",
                {
                  scope: "col",
                },
                "Valor",
              ),
              /*#__PURE__*/ React.createElement("th", {
                scope: "col",
                "aria-label": "A\xE7\xF5es",
              }),
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "tbody",
            null,
            rows.map((t, i) => {
              const ym = t.date.slice(0, 7);
              const showMonth = i === 0 || rows[i - 1].date.slice(0, 7) !== ym;
              return /*#__PURE__*/ React.createElement(
                React.Fragment,
                {
                  key: t.id,
                },
                showMonth &&
                  /*#__PURE__*/ React.createElement(
                    "tr",
                    {
                      className: "txn-month-sep",
                    },
                    /*#__PURE__*/ React.createElement(
                      "th",
                      {
                        scope: "colgroup",
                        colSpan: 6,
                      },
                      monthLabel(ym),
                    ),
                  ),
                /*#__PURE__*/ React.createElement(LedgerRow, {
                  t: t,
                  itemsOpen: itemsId === t.id,
                  actionOpen: actionId === t.id,
                  tagOpen: tagId === t.id,
                  onToggleItems: () => toggleItems(t.id),
                  onToggleAction: () => toggleAction(t.id),
                  onToggleTag: () => toggleTag(t.id),
                }),
                itemsId === t.id &&
                  /*#__PURE__*/ React.createElement(LineItemsPanel, {
                    t: t,
                  }),
                actionId === t.id &&
                  /*#__PURE__*/ React.createElement(ActionPanel, {
                    t: t,
                    onClose: () => setActionId(null),
                  }),
                tagId === t.id &&
                  /*#__PURE__*/ React.createElement(TagEditorPanel, {
                    t: t,
                    onClose: () => setTagId(null),
                  }),
              );
            }),
          ),
        );
      }

      /* ---- Formulário inline de novo lançamento (stub visual) ---- */
      function NewLancamentoForm({ onClose }) {
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            style: {
              marginBottom: "var(--space-4)",
              background: "var(--surface)",
              border: "1px solid var(--border)",
              borderRadius: "var(--radius-md)",
              padding: "var(--space-6)",
              display: "flex",
              flexDirection: "column",
              gap: "var(--space-4)",
            },
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              style: {
                fontWeight: "var(--fw-semibold)",
                fontSize: "var(--fs-sm)",
                color: "var(--text-strong)",
              },
            },
            "Novo lan\xE7amento",
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              style: {
                display: "flex",
                gap: "var(--space-4)",
                flexWrap: "wrap",
              },
            },
            [
              {
                label: "Tipo",
                placeholder: "Diário",
              },
              {
                label: "Valor (R$)",
                placeholder: "0,00",
              },
              {
                label: "Data",
                placeholder: "21/06/2026",
              },
              {
                label: "Descrição",
                placeholder: "Ex.: Farmácia",
              },
            ].map(({ label, placeholder }) =>
              /*#__PURE__*/ React.createElement(
                "label",
                {
                  key: label,
                  style: {
                    display: "flex",
                    flexDirection: "column",
                    gap: 4,
                    minWidth: 120,
                  },
                },
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    style: {
                      fontSize: "var(--fs-micro)",
                      fontWeight: "var(--fw-bold)",
                      textTransform: "uppercase",
                      letterSpacing: ".06em",
                      color: "var(--text-faint)",
                    },
                  },
                  label,
                ),
                /*#__PURE__*/ React.createElement("input", {
                  placeholder: placeholder,
                  style: {
                    height: "var(--hit-min)",
                    padding: "0 10px",
                    background: "var(--surface-2)",
                    border: "var(--bw-hair) solid var(--border)",
                    borderRadius: "var(--radius-sm)",
                    color: "var(--text)",
                    fontFamily: "var(--font-sans)",
                    fontSize: "var(--fs-sm)",
                    outline: "none",
                  },
                }),
              ),
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              style: {
                display: "flex",
                gap: "var(--space-3)",
              },
            },
            /*#__PURE__*/ React.createElement(
              Button,
              {
                size: "sm",
                variant: "primary",
              },
              "Salvar",
            ),
            /*#__PURE__*/ React.createElement(
              Button,
              {
                size: "sm",
                variant: "ghost",
                onClick: onClose,
              },
              "Cancelar",
            ),
          ),
        );
      }

      /* ---- Tela principal ---- */
      function TransactionsScreen() {
        const [scope, setScope] = React.useState("all");
        const [showForm, setShowForm] = React.useState(false);
        const filtered = React.useMemo(() => {
          const txns = [...DEMO_TRANSACTIONS].sort((a, b) =>
            b.date.localeCompare(a.date),
          );
          if (scope === "credit")
            return txns.filter((t) => t.payment_method === "credit");
          if (scope === "future") return txns.filter((t) => t.is_projection);
          return txns;
        }, [scope]);
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "dash",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "txs-tools",
            },
            /*#__PURE__*/ React.createElement(SegmentedControl, {
              size: "sm",
              ariaLabel: "Filtrar lan\xE7amentos por escopo",
              value: scope,
              onChange: setScope,
              options: [
                {
                  value: "all",
                  label: "Todas",
                },
                {
                  value: "credit",
                  label: "Crédito",
                },
                {
                  value: "future",
                  label: "Futuro",
                },
              ],
            }),
            /*#__PURE__*/ React.createElement("span", {
              className: "txs-tools__sp",
            }),
            /*#__PURE__*/ React.createElement(
              Badge,
              {
                tone: "secondary",
              },
              filtered.length,
              " ",
              filtered.length === 1 ? "exibida" : "exibidas",
            ),
            /*#__PURE__*/ React.createElement(
              Button,
              {
                size: "sm",
                variant: showForm ? "ghost" : "primary",
                iconLeft: /*#__PURE__*/ React.createElement(Icon, {
                  name: "plus",
                  size: 15,
                }),
                onClick: () => setShowForm((v) => !v),
              },
              showForm ? "Fechar" : "Novo lançamento",
            ),
          ),
          showForm &&
            /*#__PURE__*/ React.createElement(NewLancamentoForm, {
              onClose: () => setShowForm(false),
            }),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-card",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "dash-card__body",
                style: {
                  padding: 0,
                },
              },
              filtered.length === 0
                ? /*#__PURE__*/ React.createElement(EmptyState, {
                    variant: "empty",
                    title: "Nenhum lan\xE7amento encontrado",
                    description: "Nenhum resultado para o filtro atual.",
                  })
                : /*#__PURE__*/ React.createElement(LedgerTable, {
                    rows: filtered,
                  }),
            ),
          ),
        );
      }
      window.TransactionsScreen = TransactionsScreen;
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "ui_kits/transactions/TransactionsScreen.jsx",
      error: String((e && e.message) || e),
    });
  }
})();
