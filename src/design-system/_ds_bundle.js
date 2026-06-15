/* @ds-bundle: {"format":3,"namespace":"NekoFinanceDesignSystem_9bd1cd","components":[{"name":"ChatBubble","sourcePath":"components/copilot/ChatBubble.jsx"},{"name":"Citation","sourcePath":"components/copilot/Citation.jsx"},{"name":"EmptyState","sourcePath":"components/copilot/EmptyState.jsx"},{"name":"Badge","sourcePath":"components/core/Badge.jsx"},{"name":"Button","sourcePath":"components/core/Button.jsx"},{"name":"Input","sourcePath":"components/core/Input.jsx"},{"name":"SegmentedControl","sourcePath":"components/core/SegmentedControl.jsx"},{"name":"Switch","sourcePath":"components/core/Switch.jsx"},{"name":"ApprovalDiffCard","sourcePath":"components/finance/ApprovalDiffCard.jsx"},{"name":"HealthBadge","sourcePath":"components/finance/HealthBadge.jsx"},{"name":"MetricTile","sourcePath":"components/finance/MetricTile.jsx"},{"name":"OwnerChip","sourcePath":"components/finance/OwnerChip.jsx"},{"name":"TransactionRow","sourcePath":"components/finance/TransactionRow.jsx"}],"sourceHashes":{"components/copilot/ChatBubble.jsx":"29739da36968","components/copilot/Citation.jsx":"3ab039f80d3d","components/copilot/EmptyState.jsx":"9effd219dd82","components/core/Badge.jsx":"8b934d31e530","components/core/Button.jsx":"1adc0bf5632e","components/core/Input.jsx":"d72981283b66","components/core/SegmentedControl.jsx":"26a185c9b2ef","components/core/Switch.jsx":"12f0243b7fc4","components/finance/ApprovalDiffCard.jsx":"a64fe49fef72","components/finance/HealthBadge.jsx":"d0dcf508602a","components/finance/MetricTile.jsx":"1dcda9419522","components/finance/OwnerChip.jsx":"4264d73eb82a","components/finance/TransactionRow.jsx":"81ba0e2fa6ed","ui_kits/copilot/CopilotScreen.jsx":"e17082d06248","ui_kits/dashboard/AppShell.jsx":"dfd281039cb4","ui_kits/dashboard/DashboardScreen.jsx":"b3f21f786dfa","ui_kits/methodology/MethodologyScreen.jsx":"8fd77dd7b9fa","ui_kits/settings/SettingsScreen.jsx":"3583409237c8","ui_kits/shared/icons.jsx":"4478d71caef5","ui_kits/transactions/TransactionsScreen.jsx":"ab6ad439be89"},"inlinedExternals":[],"unexposedExports":[]} */

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
@media (prefers-reduced-motion:reduce){.nk-state__spin{animation-duration:2s;}}
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
                icon ||
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
      const CSS = `
.nk-badge{display:inline-flex;align-items:center;gap:6px;height:22px;padding:0 9px;border-radius:var(--radius-pill);
  font-family:var(--font-sans);font-size:11.5px;font-weight:600;letter-spacing:.01em;white-space:nowrap;
  border:1px solid transparent;line-height:1;}
.nk-badge__dot{width:6px;height:6px;border-radius:50%;flex:none;}
.nk-badge--solid{color:#fff;}
.nk-badge--square{border-radius:var(--radius-xs);}
.nk-badge--neutral{background:var(--surface-elevated);color:var(--text-muted);border-color:var(--border);}
.nk-badge--success{background:var(--success-tint);color:var(--success-400);}
.nk-badge--warning{background:var(--warning-tint);color:var(--warning-400);}
.nk-badge--danger{background:var(--danger-tint);color:var(--danger-400);}
.nk-badge--info{background:var(--info-tint);color:var(--info-400);}
.nk-badge--primary{background:var(--primary-quiet);color:var(--primary);}
`;
      function useCSS() {
        React.useEffect(() => {
          if (document.getElementById("nk-badge-css")) return;
          const s = document.createElement("style");
          s.id = "nk-badge-css";
          s.textContent = CSS;
          document.head.appendChild(s);
        }, []);
      }
      const DOTS = {
        success: "var(--success-500)",
        warning: "var(--warning-500)",
        danger: "var(--danger-500)",
        info: "var(--info-500)",
        primary: "var(--primary)",
        neutral: "var(--text-faint)",
      };
      function Badge({
        tone = "neutral",
        dot = false,
        square = false,
        children,
        className = "",
        ...rest
      }) {
        useCSS();
        return /*#__PURE__*/ React.createElement(
          "span",
          _extends(
            {
              className: [
                "nk-badge",
                `nk-badge--${tone}`,
                square ? "nk-badge--square" : "",
                className,
              ]
                .filter(Boolean)
                .join(" "),
            },
            rest,
          ),
          dot
            ? /*#__PURE__*/ React.createElement("span", {
                className: "nk-badge__dot",
                style: {
                  background: DOTS[tone],
                },
              })
            : null,
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
.nk-btn{--_h:36px;--_px:14px;--_fs:14px;display:inline-flex;align-items:center;justify-content:center;gap:8px;
  height:var(--_h);padding:0 var(--_px);font-family:var(--font-sans);font-size:var(--_fs);font-weight:600;
  line-height:1;border-radius:var(--radius-sm);border:1px solid transparent;cursor:pointer;white-space:nowrap;
  letter-spacing:-0.005em;transition:var(--t-hover),transform var(--dur-instant) var(--ease-standard);
  -webkit-tap-highlight-color:transparent;user-select:none;}
.nk-btn:active{transform:translateY(0.5px) scale(0.992);}
.nk-btn:focus-visible{outline:none;box-shadow:0 0 0 2px var(--bg),0 0 0 4px var(--focus-ring);}
.nk-btn[disabled]{opacity:.45;cursor:not-allowed;transform:none;}
.nk-btn--sm{--_h:30px;--_px:11px;--_fs:13px;}
.nk-btn--lg{--_h:44px;--_px:20px;--_fs:15px;}
.nk-btn--full{width:100%;}
.nk-btn__ic{display:inline-flex;width:16px;height:16px;flex:none;}
.nk-btn--primary{background:var(--primary);color:var(--text-on-primary);}
.nk-btn--primary:hover:not([disabled]){background:var(--primary-hover);}
.nk-btn--primary:active:not([disabled]){background:var(--primary-press);}
.nk-btn--secondary{background:var(--surface-elevated);color:var(--text);border-color:var(--border-strong);}
.nk-btn--secondary:hover:not([disabled]){background:var(--surface-hover);border-color:var(--border-strong);}
.nk-btn--ghost{background:transparent;color:var(--text-muted);}
.nk-btn--ghost:hover:not([disabled]){background:var(--surface-hover);color:var(--text);}
.nk-btn--danger{background:var(--danger-500);color:#fff;}
.nk-btn--danger:hover:not([disabled]){filter:brightness(1.08);}
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
        fullWidth = false,
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
          fullWidth ? "nk-btn--full" : "",
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
      const CSS = `
.nk-field{display:flex;flex-direction:column;gap:6px;font-family:var(--font-sans);}
.nk-field__label{font-size:12px;font-weight:600;color:var(--text-muted);letter-spacing:.01em;}
.nk-field__req{color:var(--danger-400);margin-left:3px;}
.nk-input{display:flex;align-items:center;gap:8px;height:36px;padding:0 11px;background:var(--surface);
  border:1px solid var(--border);border-radius:var(--radius-sm);transition:var(--t-hover),box-shadow var(--dur-fast) var(--ease-standard);}
.nk-input:hover{border-color:var(--border-strong);}
.nk-input:focus-within{border-color:var(--border-focus);box-shadow:0 0 0 3px var(--focus-ring);}
.nk-input--err{border-color:var(--danger-500);}
.nk-input--err:focus-within{box-shadow:0 0 0 3px var(--danger-tint);}
.nk-input input{flex:1;min-width:0;background:none;border:none;outline:none;color:var(--text);
  font-family:inherit;font-size:14px;}
.nk-input input::placeholder{color:var(--text-faint);}
.nk-input--money input{font-family:var(--font-money);font-variant-numeric:tabular-nums;text-align:right;}
.nk-input__affix{color:var(--text-faint);font-size:13px;display:inline-flex;align-items:center;flex:none;}
.nk-input__icon{width:16px;height:16px;color:var(--text-faint);flex:none;display:inline-flex;}
.nk-input[disabled],.nk-input--disabled{opacity:.5;pointer-events:none;}
.nk-field__hint{font-size:11.5px;color:var(--text-faint);}
.nk-field__hint--err{color:var(--danger-400);}
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
        error = "",
        hint = "",
        disabled = false,
        className = "",
        id,
        ...rest
      }) {
        useCSS();
        const fid = id || React.useId();
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

  // components/core/SegmentedControl.jsx
  try {
    (() => {
      const CSS = `
.nk-seg{display:inline-flex;padding:3px;background:var(--surface);border:1px solid var(--border);
  border-radius:var(--radius-sm);gap:2px;font-family:var(--font-sans);}
.nk-seg__opt{appearance:none;border:none;background:none;cursor:pointer;height:28px;padding:0 13px;
  border-radius:4px;font-size:13px;font-weight:600;color:var(--text-muted);white-space:nowrap;
  display:inline-flex;align-items:center;gap:7px;transition:var(--t-hover);}
.nk-seg__opt:hover{color:var(--text);}
.nk-seg__opt[aria-selected="true"]{background:var(--surface-elevated);color:var(--text-strong);
  box-shadow:var(--shadow-1);}
.nk-seg__opt:focus-visible{outline:none;box-shadow:0 0 0 2px var(--bg),0 0 0 4px var(--focus-ring);}
.nk-seg__dot{width:8px;height:8px;border-radius:50%;flex:none;}
.nk-seg--sm .nk-seg__opt{height:24px;padding:0 10px;font-size:12px;}
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
        options = [],
        value,
        onChange = () => {},
        size = "md",
        className = "",
      }) {
        useCSS();
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            role: "tablist",
            className: ["nk-seg", size === "sm" ? "nk-seg--sm" : "", className]
              .filter(Boolean)
              .join(" "),
          },
          options.map((o) => {
            const opt =
              typeof o === "string"
                ? {
                    value: o,
                    label: o,
                  }
                : o;
            const selected = opt.value === value;
            return /*#__PURE__*/ React.createElement(
              "button",
              {
                key: opt.value,
                role: "tab",
                type: "button",
                "aria-selected": selected,
                className: "nk-seg__opt",
                onClick: () => onChange(opt.value),
              },
              opt.dot
                ? /*#__PURE__*/ React.createElement("span", {
                    className: "nk-seg__dot",
                    style: {
                      background: opt.dot,
                    },
                  })
                : null,
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
  font-size:13px;color:var(--text);user-select:none;}
.nk-switch input{position:absolute;opacity:0;width:0;height:0;}
.nk-switch__track{position:relative;width:38px;height:22px;border-radius:var(--radius-pill);
  background:var(--ink-600);border:1px solid var(--border-strong);transition:var(--t-hover);flex:none;}
.nk-switch__thumb{position:absolute;top:2px;left:2px;width:16px;height:16px;border-radius:50%;
  background:var(--text-muted);box-shadow:var(--shadow-1);transition:transform var(--dur-base) var(--ease-standard),background var(--dur-fast) var(--ease-standard);}
.nk-switch input:checked + .nk-switch__track{background:var(--primary);border-color:var(--primary);}
.nk-switch input:checked + .nk-switch__track .nk-switch__thumb{transform:translateX(16px);background:var(--text-on-primary);}
.nk-switch input:focus-visible + .nk-switch__track{box-shadow:0 0 0 2px var(--bg),0 0 0 4px var(--focus-ring);}
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
        checked,
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
.nk-diff{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);
  overflow:hidden;font-family:var(--font-sans);box-shadow:var(--shadow-2);max-width:480px;}
.nk-diff__head{display:flex;align-items:flex-start;gap:11px;padding:14px 16px;border-bottom:1px solid var(--border);}
.nk-diff__mark{width:28px;height:28px;border-radius:var(--radius-sm);background:var(--primary-quiet);color:var(--primary);
  display:flex;align-items:center;justify-content:center;flex:none;}
.nk-diff__htxt{flex:1;min-width:0;}
.nk-diff__title{font-size:14px;font-weight:700;color:var(--text-strong);letter-spacing:-0.005em;}
.nk-diff__src{font-family:var(--font-money);font-size:11px;color:var(--text-faint);margin-top:3px;
  display:flex;align-items:center;gap:6px;flex-wrap:wrap;}
.nk-diff__src b{color:var(--text-muted);font-weight:600;}
.nk-diff__pill{font-size:10px;font-weight:700;letter-spacing:.05em;text-transform:uppercase;padding:3px 8px;
  border-radius:var(--radius-pill);flex:none;}
.nk-diff__pill--pending{background:var(--warning-tint);color:var(--warning-400);}
.nk-diff__pill--approved{background:var(--success-tint);color:var(--success-400);}
.nk-diff__pill--rejected{background:var(--danger-tint);color:var(--danger-400);}
.nk-diff__rows{padding:6px 16px 12px;}
.nk-diff__row{display:grid;grid-template-columns:104px 1fr;gap:10px;padding:8px 0;border-bottom:1px dashed var(--border);align-items:center;}
.nk-diff__row:last-child{border-bottom:none;}
.nk-diff__field{font-size:12px;color:var(--text-muted);font-weight:600;}
.nk-diff__vals{display:flex;align-items:center;gap:8px;flex-wrap:wrap;font-family:var(--font-money);
  font-variant-numeric:tabular-nums;font-size:13px;}
.nk-diff__before{color:var(--danger-400);background:var(--danger-tint);padding:2px 7px;border-radius:var(--radius-xs);
  text-decoration:line-through;text-decoration-thickness:1px;}
.nk-diff__arrow{color:var(--text-faint);}
.nk-diff__after{color:var(--success-400);background:var(--success-tint);padding:2px 7px;border-radius:var(--radius-xs);font-weight:600;}
.nk-diff__note{display:flex;gap:8px;padding:11px 16px;background:var(--bg-subtle);border-top:1px solid var(--border);
  font-size:12px;line-height:1.45;color:var(--text-muted);}
.nk-diff__note b{color:var(--text);font-weight:600;}
.nk-diff__actions{display:flex;gap:8px;padding:12px 16px;border-top:1px solid var(--border);}
.nk-diff__spacer{flex:1;}
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
        pending: "Needs approval",
        approved: "Approved",
        rejected: "Rejected",
      };
      function ApprovalDiffCard({
        title = "Proposed change",
        sheet,
        range,
        changes = [],
        note = null,
        status = "pending",
        actions = null,
        className = "",
      }) {
        useCSS();
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: ["nk-diff", className].filter(Boolean).join(" "),
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
                className: `nk-diff__pill nk-diff__pill--${status}`,
              },
              PILL[status],
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
                  key: i,
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
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    "aria-hidden": "true",
                    style: {
                      color: "var(--primary)",
                    },
                  },
                  /*#__PURE__*/ React.createElement(
                    "svg",
                    {
                      width: "15",
                      height: "15",
                      viewBox: "0 0 24 24",
                      fill: "none",
                      stroke: "currentColor",
                      strokeWidth: "2",
                      strokeLinecap: "round",
                      strokeLinejoin: "round",
                    },
                    /*#__PURE__*/ React.createElement("circle", {
                      cx: "12",
                      cy: "12",
                      r: "9",
                    }),
                    /*#__PURE__*/ React.createElement("path", {
                      d: "M12 8h.01M11 12h1v4h1",
                    }),
                  ),
                ),
                /*#__PURE__*/ React.createElement("span", null, note),
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

  // components/finance/HealthBadge.jsx
  try {
    (() => {
      const CSS = `
.nk-health{display:inline-flex;align-items:center;gap:10px;padding:7px 13px 7px 9px;border-radius:var(--radius-pill);
  font-family:var(--font-sans);border:1px solid transparent;line-height:1;}
.nk-health__ring{width:24px;height:24px;flex:none;transform:rotate(-90deg);}
.nk-health__txt{display:flex;flex-direction:column;gap:2px;}
.nk-health__label{font-size:13px;font-weight:700;letter-spacing:-0.005em;}
.nk-health__sub{font-size:10.5px;font-weight:500;opacity:.8;}
.nk-health--strong{background:var(--success-tint);border-color:rgba(52,185,129,.25);color:var(--success-400);}
.nk-health--steady{background:var(--primary-quiet);border-color:rgba(63,191,143,.22);color:var(--primary);}
.nk-health--watch{background:var(--warning-tint);border-color:rgba(224,163,62,.25);color:var(--warning-400);}
.nk-health--risk{background:var(--danger-tint);border-color:rgba(224,98,91,.25);color:var(--danger-400);}
.nk-health--lg{padding:10px 18px 10px 12px;}
.nk-health--lg .nk-health__ring{width:34px;height:34px;}
.nk-health--lg .nk-health__label{font-size:16px;}
`;
      function useCSS() {
        React.useEffect(() => {
          if (document.getElementById("nk-health-css")) return;
          const s = document.createElement("style");
          s.id = "nk-health-css";
          s.textContent = CSS;
          document.head.appendChild(s);
        }, []);
      }
      const LABELS = {
        strong: "Strong",
        steady: "Steady",
        watch: "Watch",
        risk: "At risk",
      };
      function HealthBadge({
        level = "steady",
        score = null,
        sublabel = "",
        size = "md",
        className = "",
      }) {
        useCSS();
        const pct =
          score == null
            ? {
                strong: 92,
                steady: 74,
                watch: 48,
                risk: 24,
              }[level]
            : score;
        const r = size === "lg" ? 15 : 10;
        const c = 2 * Math.PI * r;
        const dim = size === "lg" ? 34 : 24;
        const cx = dim / 2;
        return /*#__PURE__*/ React.createElement(
          "span",
          {
            className: [
              "nk-health",
              `nk-health--${level}`,
              size === "lg" ? "nk-health--lg" : "",
              className,
            ]
              .filter(Boolean)
              .join(" "),
          },
          /*#__PURE__*/ React.createElement(
            "svg",
            {
              className: "nk-health__ring",
              viewBox: `0 0 ${dim} ${dim}`,
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
            }),
          ),
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: "nk-health__txt",
            },
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "nk-health__label",
              },
              LABELS[level],
            ),
            sublabel
              ? /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "nk-health__sub",
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

  // components/finance/MetricTile.jsx
  try {
    (() => {
      const CSS = `
.nk-tile{display:flex;flex-direction:column;gap:10px;padding:16px 18px;background:var(--surface);
  border:1px solid var(--border);border-radius:var(--radius-md);box-shadow:var(--shadow-1);min-width:0;}
.nk-tile__top{display:flex;align-items:center;justify-content:space-between;gap:10px;}
.nk-tile__label{font-family:var(--font-sans);font-size:12px;font-weight:600;color:var(--text-muted);
  letter-spacing:.01em;display:flex;align-items:center;gap:7px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;min-width:0;}
.nk-tile__ic{width:15px;height:15px;color:var(--text-faint);display:inline-flex;}
.nk-tile__val{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-weight:600;
  font-size:var(--fs-money-lg);letter-spacing:-0.01em;color:var(--text-strong);line-height:1.05;}
.nk-tile__val .cents{color:var(--text-faint);}
.nk-tile__foot{display:flex;align-items:center;gap:8px;}
.nk-tile__delta{display:inline-flex;align-items:center;gap:4px;font-family:var(--font-money);
  font-variant-numeric:tabular-nums;font-size:12.5px;font-weight:600;}
.nk-tile__delta--up{color:var(--money-pos);}
.nk-tile__delta--down{color:var(--money-neg);}
.nk-tile__delta--flat{color:var(--text-muted);}
.nk-tile__sub{font-family:var(--font-sans);font-size:11.5px;color:var(--text-faint);}
.nk-tile__spark{display:flex;align-items:flex-end;gap:2px;height:24px;}
.nk-tile__spark span{width:4px;border-radius:1px;background:var(--primary);opacity:.55;}
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
      function splitMoney(v) {
        const str = String(v);
        const dot = str.lastIndexOf(".");
        if (dot === -1) return [str, ""];
        return [str.slice(0, dot), str.slice(dot)];
      }
      function MetricTile({
        label,
        value,
        icon = null,
        delta = null,
        deltaDir = "up",
        sublabel = "",
        spark = null,
        className = "",
      }) {
        useCSS();
        const [whole, cents] = splitMoney(value);
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: ["nk-tile", className].filter(Boolean).join(" "),
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "nk-tile__top",
            },
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "nk-tile__label",
              },
              icon
                ? /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      className: "nk-tile__ic",
                    },
                    icon,
                  )
                : null,
              label,
            ),
            spark
              ? /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "nk-tile__spark",
                  },
                  spark.map((h, i) =>
                    /*#__PURE__*/ React.createElement("span", {
                      key: i,
                      style: {
                        height: `${Math.max(8, h)}%`,
                      },
                    }),
                  ),
                )
              : null,
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "nk-tile__val",
            },
            whole,
            cents
              ? /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "cents",
                  },
                  cents,
                )
              : null,
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
                      },
                      deltaDir === "up" ? "▲" : deltaDir === "down" ? "▼" : "▬",
                      " ",
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

  // components/finance/OwnerChip.jsx
  try {
    (() => {
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
      function OwnerChip({
        name,
        type = "personal",
        role = null,
        bare = false,
        className = "",
      }) {
        useCSS();
        const isShared = type === "shared";
        return /*#__PURE__*/ React.createElement(
          "span",
          {
            className: [
              "nk-owner",
              `nk-owner--${type}`,
              bare ? "nk-owner--bare" : "",
              className,
            ]
              .filter(Boolean)
              .join(" "),
          },
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: ["nk-owner__av", isShared ? "nk-owner__av--split" : ""]
                .filter(Boolean)
                .join(" "),
            },
            isShared ? "◐" : initials(name),
          ),
          /*#__PURE__*/ React.createElement("span", null, name),
          role
            ? /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "nk-owner__role",
                },
                role,
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

  // components/finance/TransactionRow.jsx
  try {
    (() => {
      const CSS = `
.nk-txn{display:grid;grid-template-columns:84px minmax(0,1fr) auto auto 132px;align-items:center;gap:14px;
  padding:0 14px;height:var(--row-h-default);border-bottom:1px solid var(--border);font-family:var(--font-sans);
  cursor:default;transition:background var(--dur-fast) var(--ease-standard);}
.nk-txn:hover{background:var(--surface-hover);}
.nk-txn--selected{background:var(--surface-selected);box-shadow:inset 2px 0 0 var(--primary);}
.nk-txn--flag{box-shadow:inset 2px 0 0 var(--warning-500);}
.nk-txn__date{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-size:12px;color:var(--text-faint);}
.nk-txn__main{min-width:0;display:flex;flex-direction:column;gap:2px;}
.nk-txn__merchant{font-size:13.5px;font-weight:600;color:var(--text);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;}
.nk-txn__cat{display:inline-flex;align-items:center;gap:6px;font-size:11.5px;color:var(--text-muted);}
.nk-txn__catdot{width:7px;height:7px;border-radius:2px;flex:none;}
.nk-txn__owner{display:flex;justify-content:flex-end;}
.nk-txn__status{display:flex;align-items:center;gap:6px;font-size:11px;font-weight:600;justify-content:flex-end;min-width:96px;}
.nk-txn__dot{width:7px;height:7px;border-radius:50%;flex:none;}
.nk-txn__conf{display:inline-flex;gap:2px;align-items:center;}
.nk-txn__conf i{width:3px;border-radius:1px;background:currentColor;display:inline-block;}
.nk-txn__amt{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-size:14px;font-weight:600;text-align:right;}
.nk-txn__amt--pos{color:var(--money-pos);}
.nk-txn__amt--neg{color:var(--text);}
`;
      function useCSS() {
        React.useEffect(() => {
          if (document.getElementById("nk-txn-css")) return;
          const s = document.createElement("style");
          s.id = "nk-txn-css";
          s.textContent = CSS;
          document.head.appendChild(s);
        }, []);
      }
      const STATUS = {
        reconciled: {
          c: "var(--success-500)",
          t: "var(--success-400)",
          label: "Reconciled",
        },
        imported: {
          c: "var(--info-500)",
          t: "var(--info-400)",
          label: "Imported",
        },
        "needs-owner": {
          c: "var(--warning-500)",
          t: "var(--warning-400)",
          label: "Needs owner",
        },
      };
      function TransactionRow({
        date,
        merchant,
        category,
        categoryColor = "var(--chart-3)",
        owner = null,
        amount,
        positive = false,
        status = "reconciled",
        confidence = null,
        selected = false,
        onClick,
        className = "",
      }) {
        useCSS();
        const st = STATUS[status] || STATUS.reconciled;
        const flag = status === "needs-owner";
        const bars =
          {
            high: 3,
            medium: 2,
            low: 1,
          }[confidence] || 0;
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: [
              "nk-txn",
              selected ? "nk-txn--selected" : "",
              flag && !selected ? "nk-txn--flag" : "",
              className,
            ]
              .filter(Boolean)
              .join(" "),
            onClick: onClick,
          },
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: "nk-txn__date",
            },
            date,
          ),
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: "nk-txn__main",
            },
            /*#__PURE__*/ React.createElement(
              "span",
              {
                className: "nk-txn__merchant",
              },
              merchant,
            ),
            category
              ? /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "nk-txn__cat",
                  },
                  /*#__PURE__*/ React.createElement("span", {
                    className: "nk-txn__catdot",
                    style: {
                      background: categoryColor,
                    },
                  }),
                  category,
                )
              : null,
          ),
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: "nk-txn__owner",
            },
            owner,
          ),
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: "nk-txn__status",
              style: {
                color: st.t,
              },
            },
            confidence
              ? /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "nk-txn__conf",
                    title: `${confidence} confidence`,
                  },
                  [0, 1, 2].map((i) =>
                    /*#__PURE__*/ React.createElement("i", {
                      key: i,
                      style: {
                        height: `${6 + i * 3}px`,
                        opacity: i < bars ? 1 : 0.25,
                      },
                    }),
                  ),
                )
              : /*#__PURE__*/ React.createElement("span", {
                  className: "nk-txn__dot",
                  style: {
                    background: st.c,
                  },
                }),
            st.label,
          ),
          /*#__PURE__*/ React.createElement(
            "span",
            {
              className: `nk-txn__amt nk-txn__amt--${positive ? "pos" : "neg"}`,
            },
            positive ? "+ " : "",
            amount,
          ),
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

  // ui_kits/copilot/CopilotScreen.jsx
  try {
    (() => {
      /* Neko Finance — Copilot approval flow. Chat with cited/deterministic answers
   and a live human-approved sheet diff. Exposes window.CopilotApp. */
      const CP_NS = window.NekoFinanceDesignSystem_9bd1cd;
      const { ChatBubble, Citation, ApprovalDiffCard, Button, Badge, Input } = CP_NS;
      const CpIcon = window.Icon;
      const cpCSS = `
.cp{display:flex;flex-direction:column;height:100%;min-height:0;}
.cp-scroll{flex:1;overflow:auto;display:flex;flex-direction:column;align-items:center;padding:8px 0 18px;}
.cp-thread{width:100%;max-width:720px;display:flex;flex-direction:column;gap:16px;padding:0 22px;}
.cp-day{align-self:center;font-size:11px;color:var(--text-faint);background:var(--surface);border:1px solid var(--border);
  padding:3px 11px;border-radius:999px;margin:4px 0;}
.cp-approved{display:flex;align-items:center;gap:8px;font-size:12.5px;color:var(--success-400);font-weight:600;
  padding-left:42px;}
.cp-composer{flex:none;border-top:1px solid var(--border);background:var(--bg-subtle);padding:14px 22px;}
.cp-composer__inner{max-width:720px;margin:0 auto;display:flex;flex-direction:column;gap:8px;}
.cp-inrow{display:flex;align-items:flex-end;gap:9px;background:var(--surface);border:1px solid var(--border);
  border-radius:var(--radius-md);padding:8px 8px 8px 14px;transition:border-color var(--dur-fast) var(--ease-standard);}
.cp-inrow:focus-within{border-color:var(--border-focus);}
.cp-inrow textarea{flex:1;resize:none;border:none;outline:none;background:none;color:var(--text);font-family:var(--font-sans);
  font-size:14px;line-height:1.45;max-height:120px;padding:5px 0;}
.cp-inrow textarea::placeholder{color:var(--text-faint);}
.cp-send{width:34px;height:34px;border-radius:var(--radius-sm);border:none;background:var(--primary);color:var(--text-on-primary);
  display:flex;align-items:center;justify-content:center;cursor:pointer;flex:none;transition:var(--t-hover);}
.cp-send:hover{background:var(--primary-hover);}
.cp-foot{display:flex;align-items:center;gap:8px;justify-content:center;font-size:11px;color:var(--text-faint);}
.cp-foot__dot{width:5px;height:5px;border-radius:50%;background:var(--success-500);}
/* dock */
.cpd{padding:16px;display:flex;flex-direction:column;gap:16px;}
.cpd-h{font-size:11px;font-weight:700;letter-spacing:.07em;text-transform:uppercase;color:var(--text-faint);}
.cpd-card{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);padding:13px 14px;}
.cpd-row{display:flex;align-items:flex-start;gap:10px;padding:7px 0;}
.cpd-row__ic{width:26px;height:26px;border-radius:7px;display:flex;align-items:center;justify-content:center;flex:none;}
.cpd-row__t{font-size:12.5px;font-weight:600;color:var(--text);}
.cpd-row__s{font-size:11px;color:var(--text-faint);margin-top:1px;line-height:1.35;}
.cpd-priv{display:flex;align-items:center;gap:9px;padding:11px 13px;background:var(--primary-quiet);
  border:1px solid rgba(63,191,143,.22);border-radius:var(--radius-md);}
.cpd-priv__t{font-size:12px;font-weight:700;color:var(--text-strong);}
.cpd-priv__s{font-size:11px;color:var(--text-muted);margin-top:1px;}
`;
      function injectCp() {
        if (document.getElementById("cp-css")) return;
        const s = document.createElement("style");
        s.id = "cp-css";
        s.textContent = cpCSS;
        document.head.appendChild(s);
      }
      function CopilotApp() {
        injectCp();
        const [nav, setNav] = React.useState("copilot");
        const [status, setStatus] = React.useState("pending"); // pending|approved|rejected
        const scrollRef = React.useRef(null);
        const dock = /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "cpd",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "cpd-priv",
            },
            /*#__PURE__*/ React.createElement(
              "span",
              {
                style: {
                  color: "var(--primary)",
                },
              },
              /*#__PURE__*/ React.createElement(CpIcon, {
                name: "lock",
                size: 20,
              }),
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              null,
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "cpd-priv__t",
                },
                "Private & local",
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "cpd-priv__s",
                },
                "Mia runs on-device. Nothing leaves your machine without approval.",
              ),
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            null,
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "cpd-h",
                style: {
                  marginBottom: 8,
                },
              },
              "What Mia can see",
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "cpd-card",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "cpd-row",
                },
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "cpd-row__ic",
                    style: {
                      background: "var(--success-tint)",
                      color: "var(--success-400)",
                    },
                  },
                  /*#__PURE__*/ React.createElement(CpIcon, {
                    name: "table",
                    size: 15,
                  }),
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  null,
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "cpd-row__t",
                    },
                    "Expenses 2025",
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "cpd-row__s",
                    },
                    "Read-only \xB7 248 rows \xB7 synced 2m ago",
                  ),
                ),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "cpd-row",
                },
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "cpd-row__ic",
                    style: {
                      background: "var(--surface-elevated)",
                      color: "var(--text-muted)",
                    },
                  },
                  /*#__PURE__*/ React.createElement(CpIcon, {
                    name: "key",
                    size: 15,
                  }),
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  null,
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "cpd-row__t",
                    },
                    "Writes need approval",
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "cpd-row__s",
                    },
                    "Every change is shown as a diff first.",
                  ),
                ),
              ),
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            null,
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "cpd-h",
                style: {
                  marginBottom: 8,
                },
              },
              "Suggested",
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "cpd-card",
                style: {
                  display: "flex",
                  flexDirection: "column",
                  gap: 7,
                },
              },
              [
                "Why was June cashflow lower?",
                "Split rent 60/40 going forward",
                "Find subscriptions we forgot",
              ].map((s) =>
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    key: s,
                    style: {
                      fontSize: 12.5,
                      color: "var(--text-muted)",
                      cursor: "pointer",
                    },
                  },
                  "\u21B3 ",
                  s,
                ),
              ),
            ),
          ),
        );
        return /*#__PURE__*/ React.createElement(
          window.AppShell,
          {
            active: nav,
            onNav: (k) => (window.__nekoRoute ? window.__nekoRoute(k) : setNav(k)),
            title: "Ask Mia",
            crumb: "Private \xB7 runs locally",
            dock: dock,
            flush: true,
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "cp",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "cp-scroll",
                ref: scrollRef,
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "cp-thread",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "cp-day",
                  },
                  "Today",
                ),
                /*#__PURE__*/ React.createElement(
                  ChatBubble,
                  {
                    from: "user",
                    userInitials: "AT",
                  },
                  "Last month has a bunch of uncategorized dining. Can you categorize them and mark the shared ones as household?",
                ),
                /*#__PURE__*/ React.createElement(
                  ChatBubble,
                  {
                    from: "mia",
                  },
                  /*#__PURE__*/ React.createElement(
                    "p",
                    null,
                    "I found ",
                    /*#__PURE__*/ React.createElement(
                      "b",
                      null,
                      "3 uncategorized dining charges",
                    ),
                    " in May totaling ",
                    /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        className: "nk-chat__money",
                      },
                      "$131.70",
                    ),
                    " ",
                    /*#__PURE__*/ React.createElement(Citation, {
                      index: 1,
                      source: "rows 1204\u20131206",
                    }),
                    ". Two are at venues you usually split with Sam.",
                  ),
                  /*#__PURE__*/ React.createElement(Citation, {
                    variant: "tool",
                    fn: "filter(May 2025, category = \u2205, mcc = dining)",
                    lines: [
                      {
                        label: "Whole Foods · 12 May",
                        value: "78.00",
                      },
                      {
                        label: "Blue Bottle · 19 May",
                        value: "9.50",
                      },
                      {
                        label: "Bottega · 24 May",
                        value: "44.20",
                      },
                    ],
                    total: {
                      label: "Matched",
                      value: "$131.70",
                    },
                    source:
                      "Sheet \u2018Expenses 2025\u2019 \xB7 3 rows \xB7 deterministic",
                  }),
                ),
                /*#__PURE__*/ React.createElement(
                  ChatBubble,
                  {
                    from: "mia",
                  },
                  /*#__PURE__*/ React.createElement(
                    "p",
                    null,
                    "Here's the change I'd make. It needs your approval before I touch the sheet:",
                  ),
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    style: {
                      paddingLeft: 42,
                    },
                  },
                  /*#__PURE__*/ React.createElement(ApprovalDiffCard, {
                    title: "Categorize & assign 3 dining rows",
                    sheet: "Expenses 2025",
                    range: "E1204:F1206",
                    status: status,
                    changes: [
                      {
                        field: "Category",
                        before: "—",
                        after: "Dining",
                      },
                      {
                        field: "Owner (2 rows)",
                        before: "Unassigned",
                        after: "Household (shared)",
                      },
                      {
                        field: "Owner (1 row)",
                        before: "Unassigned",
                        after: "Alex (personal)",
                      },
                    ],
                    note: /*#__PURE__*/ React.createElement(
                      "span",
                      null,
                      "Venues ",
                      /*#__PURE__*/ React.createElement("b", null, "Whole Foods"),
                      " & ",
                      /*#__PURE__*/ React.createElement("b", null, "Bottega"),
                      " match your shared-dining rule; ",
                      /*#__PURE__*/ React.createElement("b", null, "Blue Bottle"),
                      " was a solo charge near your office.",
                    ),
                    actions:
                      status === "pending"
                        ? /*#__PURE__*/ React.createElement(
                            React.Fragment,
                            null,
                            /*#__PURE__*/ React.createElement(
                              Button,
                              {
                                variant: "primary",
                                size: "sm",
                                iconLeft: /*#__PURE__*/ React.createElement(CpIcon, {
                                  name: "check",
                                  size: 15,
                                }),
                                onClick: () => setStatus("approved"),
                              },
                              "Approve & write",
                            ),
                            /*#__PURE__*/ React.createElement(
                              Button,
                              {
                                variant: "ghost",
                                size: "sm",
                                iconLeft: /*#__PURE__*/ React.createElement(CpIcon, {
                                  name: "pencil",
                                  size: 14,
                                }),
                              },
                              "Edit",
                            ),
                            /*#__PURE__*/ React.createElement("span", {
                              style: {
                                flex: 1,
                              },
                            }),
                            /*#__PURE__*/ React.createElement(
                              Button,
                              {
                                variant: "danger",
                                size: "sm",
                                onClick: () => setStatus("rejected"),
                              },
                              "Reject",
                            ),
                          )
                        : /*#__PURE__*/ React.createElement(
                            "span",
                            {
                              style: {
                                fontSize: 12.5,
                                color:
                                  status === "approved"
                                    ? "var(--success-400)"
                                    : "var(--danger-400)",
                                fontWeight: 600,
                                display: "flex",
                                alignItems: "center",
                                gap: 7,
                              },
                            },
                            /*#__PURE__*/ React.createElement(CpIcon, {
                              name: status === "approved" ? "checkCircle" : "x",
                              size: 15,
                            }),
                            status === "approved"
                              ? "Written to Expenses 2025 · 3 rows updated"
                              : "Rejected — no changes made",
                          ),
                  }),
                ),
                status === "approved"
                  ? /*#__PURE__*/ React.createElement(
                      ChatBubble,
                      {
                        from: "mia",
                      },
                      /*#__PURE__*/ React.createElement(
                        "p",
                        null,
                        "Done \u2014 I updated ",
                        /*#__PURE__*/ React.createElement("b", null, "3 rows"),
                        " and your May dining now reads ",
                        /*#__PURE__*/ React.createElement(
                          "span",
                          {
                            className: "nk-chat__money",
                          },
                          "$486.20",
                        ),
                        ". Want me to set up a rule so future shared-venue charges auto-suggest \u201CHousehold\u201D?",
                      ),
                    )
                  : null,
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "cp-composer",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "cp-composer__inner",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "cp-inrow",
                  },
                  /*#__PURE__*/ React.createElement("textarea", {
                    rows: "1",
                    placeholder:
                      "Ask about your money \u2014 Mia cites every number\u2026",
                  }),
                  /*#__PURE__*/ React.createElement(
                    "button",
                    {
                      className: "cp-send",
                    },
                    /*#__PURE__*/ React.createElement(CpIcon, {
                      name: "send",
                      size: 16,
                    }),
                  ),
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "cp-foot",
                  },
                  /*#__PURE__*/ React.createElement("span", {
                    className: "cp-foot__dot",
                  }),
                  " Local model \xB7 reads your sheet read-only \xB7 writes always need approval",
                ),
              ),
            ),
          ),
        );
      }
      window.CopilotApp = CopilotApp;
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
        const NAV = [
          {
            key: "dashboard",
            label: "Dashboard",
            icon: "dashboard",
          },
          {
            key: "transactions",
            label: "Transactions",
            icon: "receipt",
            badge: 6,
          },
          {
            key: "copilot",
            label: "Ask Mia",
            icon: "sparkles",
          },
          {
            key: "methodology",
            label: "Methodology",
            icon: "book",
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
                  "Finances",
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
                  "System",
                ),
                React.createElement(
                  "button",
                  {
                    className:
                      "ak-item" + (active === "settings" ? " ak-item--active" : ""),
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
                      "Sheets connected",
                    ),
                    React.createElement(
                      "div",
                      {
                        className: "ak-conn__s",
                      },
                      "Synced 2m ago · read-only",
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
                    placeholder: "Search transactions, rules…",
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
      /* Neko Finance — Dashboard screen. Composes DS finance components +
   hand-built SVG charts using chart tokens. Exposes window.DashboardScreen. */
      const DASH_NS = window.NekoFinanceDesignSystem_9bd1cd;
      const {
        MetricTile,
        HealthBadge,
        OwnerChip,
        TransactionRow,
        SegmentedControl,
        Badge,
        Button,
      } = DASH_NS;
      const DashIcon = window.Icon;
      const dashCSS = `
.dash{display:flex;flex-direction:column;gap:18px;max-width:1180px;}
.dash-hero{display:flex;align-items:center;gap:18px;padding:18px 20px;background:var(--surface);
  border:1px solid var(--border);border-radius:var(--radius-lg);box-shadow:var(--shadow-1);}
.dash-hero__txt{flex:1;min-width:0;}
.dash-hero__line{font-size:15px;line-height:1.5;color:var(--text-muted);}
.dash-hero__line b{color:var(--text-strong);font-weight:700;}
.dash-hero__money{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-weight:600;color:var(--text);}
.dash-grid4{display:grid;grid-template-columns:repeat(4,1fr);gap:14px;}
.dash-2col{display:grid;grid-template-columns:1.6fr 1fr;gap:14px;}
.dash-card{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);box-shadow:var(--shadow-1);}
.dash-card__head{display:flex;align-items:center;justify-content:space-between;gap:10px;padding:14px 16px 8px;}
.dash-card__title{font-size:14px;font-weight:700;color:var(--text-strong);display:flex;align-items:center;gap:8px;}
.dash-card__ic{color:var(--text-faint);}
.dash-card__body{padding:8px 16px 16px;}
.dash-legend{display:flex;flex-direction:column;gap:9px;}
.dash-leg{display:flex;align-items:center;gap:9px;font-size:12.5px;}
.dash-leg__dot{width:9px;height:9px;border-radius:3px;flex:none;}
.dash-leg__name{color:var(--text-muted);flex:1;}
.dash-leg__amt{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-weight:600;color:var(--text);}
.dash-leg__pct{color:var(--text-faint);font-size:11px;width:34px;text-align:right;}
.dash-acct{display:flex;align-items:center;gap:12px;padding:11px 0;border-bottom:1px solid var(--border);}
.dash-acct:last-child{border-bottom:none;}
.dash-acct__ic{width:34px;height:34px;border-radius:9px;background:var(--surface-elevated);border:1px solid var(--border);
  display:flex;align-items:center;justify-content:center;color:var(--text-muted);flex:none;}
.dash-acct__nm{font-size:13px;font-weight:600;color:var(--text);}
.dash-acct__sub{font-size:11px;color:var(--text-faint);}
.dash-acct__amt{margin-left:auto;font-family:var(--font-money);font-variant-numeric:tabular-nums;font-weight:600;font-size:14px;}
.dash-split{display:flex;flex-direction:column;gap:13px;}
.dash-splitrow__top{display:flex;align-items:center;justify-content:space-between;margin-bottom:6px;}
.dash-splitrow__lbl{display:flex;align-items:center;gap:8px;font-size:12.5px;font-weight:600;color:var(--text);}
.dash-splitrow__amt{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-size:12.5px;color:var(--text-muted);}
.dash-bar{height:8px;border-radius:999px;background:var(--surface-2);overflow:hidden;}
.dash-bar__fill{height:100%;border-radius:999px;}
.dash-sectit{font-size:11px;font-weight:700;letter-spacing:.07em;text-transform:uppercase;color:var(--text-faint);}
@media (max-width:1080px){
  .dash-grid4{grid-template-columns:repeat(2,1fr);}
  .dash-2col{grid-template-columns:1fr;}
}`;
      function injectDash() {
        if (document.getElementById("dash-css")) return;
        const s = document.createElement("style");
        s.id = "dash-css";
        s.textContent = dashCSS;
        document.head.appendChild(s);
      }

      /* ---- Cashflow area + bars chart ---- */
      function CashflowChart() {
        const W = 660,
          H = 180,
          pad = {
            l: 8,
            r: 8,
            t: 10,
            b: 22,
          };
        const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"];
        const income = [6200, 6200, 6450, 6200, 6800, 6200];
        const spend = [4100, 5200, 3900, 4600, 4200, 3142];
        const max = 7200;
        const iw = W - pad.l - pad.r;
        const ih = H - pad.t - pad.b;
        const x = (i) => pad.l + (iw / (months.length - 1)) * i;
        const y = (v) => pad.t + ih * (1 - v / max);
        const linePath = (arr) =>
          arr
            .map((v, i) => `${i ? "L" : "M"}${x(i).toFixed(1)} ${y(v).toFixed(1)}`)
            .join(" ");
        const areaPath =
          linePath(income) +
          ` L${x(months.length - 1)} ${pad.t + ih} L${x(0)} ${pad.t + ih} Z`;
        const grid = [0, 0.25, 0.5, 0.75, 1];
        return /*#__PURE__*/ React.createElement(
          "svg",
          {
            viewBox: `0 0 ${W} ${H}`,
            style: {
              width: "100%",
              height: "auto",
              display: "block",
            },
          },
          grid.map((g, i) =>
            /*#__PURE__*/ React.createElement("line", {
              key: i,
              x1: pad.l,
              x2: W - pad.r,
              y1: pad.t + ih * g,
              y2: pad.t + ih * g,
              stroke: "var(--chart-grid)",
              strokeWidth: "1",
            }),
          ),
          /*#__PURE__*/ React.createElement(
            "defs",
            null,
            /*#__PURE__*/ React.createElement(
              "linearGradient",
              {
                id: "cf",
                x1: "0",
                y1: "0",
                x2: "0",
                y2: "1",
              },
              /*#__PURE__*/ React.createElement("stop", {
                offset: "0%",
                stopColor: "var(--chart-1)",
                stopOpacity: "0.22",
              }),
              /*#__PURE__*/ React.createElement("stop", {
                offset: "100%",
                stopColor: "var(--chart-1)",
                stopOpacity: "0",
              }),
            ),
          ),
          /*#__PURE__*/ React.createElement("path", {
            d: areaPath,
            fill: "url(#cf)",
          }),
          spend.map((v, i) =>
            /*#__PURE__*/ React.createElement("rect", {
              key: i,
              x: x(i) - 7,
              y: y(v),
              width: "14",
              height: pad.t + ih - y(v),
              rx: "3",
              fill: "var(--chart-2)",
              opacity: "0.55",
            }),
          ),
          /*#__PURE__*/ React.createElement("path", {
            d: linePath(income),
            fill: "none",
            stroke: "var(--chart-1)",
            strokeWidth: "2.5",
            strokeLinejoin: "round",
            strokeLinecap: "round",
          }),
          income.map((v, i) =>
            /*#__PURE__*/ React.createElement("circle", {
              key: i,
              cx: x(i),
              cy: y(v),
              r: "3",
              fill: "var(--bg)",
              stroke: "var(--chart-1)",
              strokeWidth: "2",
            }),
          ),
          months.map((m, i) =>
            /*#__PURE__*/ React.createElement(
              "text",
              {
                key: i,
                x: x(i),
                y: H - 6,
                textAnchor: "middle",
                fontSize: "10.5",
                fill: "var(--chart-axis)",
                fontFamily: "var(--font-mono)",
              },
              m,
            ),
          ),
        );
      }

      /* ---- Category donut ---- */
      function Donut({ data }) {
        const total = data.reduce((s, d) => s + d.value, 0);
        let acc = 0;
        const R = 52,
          sw = 16,
          C = 2 * Math.PI * R;
        return /*#__PURE__*/ React.createElement(
          "svg",
          {
            viewBox: "0 0 140 140",
            style: {
              width: 140,
              height: 140,
            },
          },
          /*#__PURE__*/ React.createElement(
            "g",
            {
              transform: "rotate(-90 70 70)",
            },
            data.map((d, i) => {
              const frac = d.value / total;
              const dash = `${(C * frac).toFixed(1)} ${(C * (1 - frac)).toFixed(1)}`;
              const off = -C * (acc / total);
              acc += d.value;
              return /*#__PURE__*/ React.createElement("circle", {
                key: i,
                cx: "70",
                cy: "70",
                r: R,
                fill: "none",
                stroke: d.color,
                strokeWidth: sw,
                strokeDasharray: dash,
                strokeDashoffset: off,
              });
            }),
          ),
          /*#__PURE__*/ React.createElement(
            "text",
            {
              x: "70",
              y: "65",
              textAnchor: "middle",
              fontSize: "11",
              fill: "var(--text-faint)",
              fontFamily: "var(--font-sans)",
            },
            "Spending",
          ),
          /*#__PURE__*/ React.createElement(
            "text",
            {
              x: "70",
              y: "84",
              textAnchor: "middle",
              fontSize: "17",
              fontWeight: "700",
              fill: "var(--text-strong)",
              fontFamily: "var(--font-money)",
            },
            "$3,142",
          ),
        );
      }
      function DashboardScreen({ onAskMia = () => {} }) {
        injectDash();
        const cats = [
          {
            name: "Housing",
            value: 1450,
            color: "var(--chart-1)",
          },
          {
            name: "Groceries",
            value: 642,
            color: "var(--chart-2)",
          },
          {
            name: "Transport",
            value: 380,
            color: "var(--chart-3)",
          },
          {
            name: "Subscriptions",
            value: 270,
            color: "var(--chart-4)",
          },
          {
            name: "Dining",
            value: 400,
            color: "var(--chart-5)",
          },
        ];
        const total = cats.reduce((s, c) => s + c.value, 0);
        return /*#__PURE__*/ React.createElement(
          "div",
          {
            className: "dash",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-hero",
            },
            /*#__PURE__*/ React.createElement(HealthBadge, {
              level: "strong",
              sublabel: "3.1 months runway",
              size: "lg",
            }),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "dash-hero__txt",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "dash-hero__line",
                },
                "You're ",
                /*#__PURE__*/ React.createElement("b", null, "$1,678"),
                " ahead this month. Spending is ",
                /*#__PURE__*/ React.createElement("b", null, "6% under"),
                " your average, with ",
                /*#__PURE__*/ React.createElement(
                  "span",
                  {
                    className: "dash-hero__money",
                  },
                  "$642",
                ),
                " in shared groceries awaiting an owner.",
              ),
            ),
            /*#__PURE__*/ React.createElement(
              Button,
              {
                variant: "secondary",
                iconLeft: /*#__PURE__*/ React.createElement(DashIcon, {
                  name: "sparkles",
                  size: 16,
                }),
                onClick: onAskMia,
              },
              "Ask Mia",
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-grid4",
            },
            /*#__PURE__*/ React.createElement(MetricTile, {
              label: "Net worth",
              value: "$182,400",
              icon: /*#__PURE__*/ React.createElement(DashIcon, {
                name: "trendingUp",
                size: 15,
              }),
              delta: "+1.8%",
              deltaDir: "up",
              sublabel: "this quarter",
            }),
            /*#__PURE__*/ React.createElement(MetricTile, {
              label: "Net cashflow",
              value: "$4,820.00",
              delta: "+12.4%",
              deltaDir: "up",
              sublabel: "vs. last month",
              spark: [40, 55, 48, 70, 62, 88, 100],
            }),
            /*#__PURE__*/ React.createElement(MetricTile, {
              label: "Spending",
              value: "$3,142.18",
              delta: "6.1%",
              deltaDir: "down",
              sublabel: "under budget",
            }),
            /*#__PURE__*/ React.createElement(MetricTile, {
              label: "Savings rate",
              value: "34%",
              delta: "+3 pts",
              deltaDir: "up",
              sublabel: "of net income",
            }),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-2col",
            },
            /*#__PURE__*/ React.createElement(
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
                  /*#__PURE__*/ React.createElement(DashIcon, {
                    name: "trendingUp",
                    size: 16,
                    className: "dash-card__ic",
                  }),
                  "Cashflow",
                ),
                /*#__PURE__*/ React.createElement(SegmentedControl, {
                  size: "sm",
                  value: "6m",
                  onChange: () => {},
                  options: [
                    {
                      value: "3m",
                      label: "3M",
                    },
                    {
                      value: "6m",
                      label: "6M",
                    },
                    {
                      value: "1y",
                      label: "1Y",
                    },
                  ],
                }),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "dash-card__body",
                },
                /*#__PURE__*/ React.createElement(CashflowChart, null),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    style: {
                      display: "flex",
                      gap: 18,
                      marginTop: 6,
                      fontSize: 11.5,
                      color: "var(--text-muted)",
                    },
                  },
                  /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      style: {
                        display: "flex",
                        alignItems: "center",
                        gap: 6,
                      },
                    },
                    /*#__PURE__*/ React.createElement("span", {
                      style: {
                        width: 14,
                        height: 3,
                        borderRadius: 2,
                        background: "var(--chart-1)",
                      },
                    }),
                    "Income",
                  ),
                  /*#__PURE__*/ React.createElement(
                    "span",
                    {
                      style: {
                        display: "flex",
                        alignItems: "center",
                        gap: 6,
                      },
                    },
                    /*#__PURE__*/ React.createElement("span", {
                      style: {
                        width: 10,
                        height: 10,
                        borderRadius: 2,
                        background: "var(--chart-2)",
                        opacity: 0.55,
                      },
                    }),
                    "Spending",
                  ),
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement(
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
                  /*#__PURE__*/ React.createElement(DashIcon, {
                    name: "piggy",
                    size: 16,
                    className: "dash-card__ic",
                  }),
                  "By category",
                ),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "dash-card__body",
                  style: {
                    display: "flex",
                    gap: 14,
                    alignItems: "center",
                  },
                },
                /*#__PURE__*/ React.createElement(Donut, {
                  data: cats,
                }),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "dash-legend",
                    style: {
                      flex: 1,
                    },
                  },
                  cats.map((c) =>
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "dash-leg",
                        key: c.name,
                      },
                      /*#__PURE__*/ React.createElement("span", {
                        className: "dash-leg__dot",
                        style: {
                          background: c.color,
                        },
                      }),
                      /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          className: "dash-leg__name",
                        },
                        c.name,
                      ),
                      /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          className: "dash-leg__amt",
                        },
                        "$",
                        c.value.toLocaleString(),
                      ),
                      /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          className: "dash-leg__pct",
                        },
                        Math.round((c.value / total) * 100),
                        "%",
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "dash-2col",
            },
            /*#__PURE__*/ React.createElement(
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
                  /*#__PURE__*/ React.createElement(DashIcon, {
                    name: "wallet",
                    size: 16,
                    className: "dash-card__ic",
                  }),
                  "Accounts & cards",
                ),
                /*#__PURE__*/ React.createElement(
                  Button,
                  {
                    variant: "ghost",
                    size: "sm",
                    iconLeft: /*#__PURE__*/ React.createElement(DashIcon, {
                      name: "plus",
                      size: 15,
                    }),
                  },
                  "Add",
                ),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "dash-card__body",
                  style: {
                    paddingTop: 0,
                  },
                },
                [
                  {
                    nm: "Joint Checking",
                    sub: "Chase ·· 4821",
                    amt: "$12,408.52",
                    ic: "wallet",
                    owner: "shared",
                  },
                  {
                    nm: "Alex — Savings",
                    sub: "Ally ·· 9920",
                    amt: "$96,140.00",
                    ic: "piggy",
                    owner: "personal",
                  },
                  {
                    nm: "Sam — Amex Gold",
                    sub: "Credit ·· 1007",
                    amt: "−$1,344.18",
                    ic: "creditCard",
                    owner: "partner",
                  },
                ].map((a) =>
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "dash-acct",
                      key: a.nm,
                    },
                    /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        className: "dash-acct__ic",
                      },
                      /*#__PURE__*/ React.createElement(DashIcon, {
                        name: a.ic,
                        size: 17,
                      }),
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      null,
                      /*#__PURE__*/ React.createElement(
                        "div",
                        {
                          className: "dash-acct__nm",
                        },
                        a.nm,
                      ),
                      /*#__PURE__*/ React.createElement(
                        "div",
                        {
                          className: "dash-acct__sub",
                        },
                        a.sub,
                      ),
                    ),
                    /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        className: "dash-acct__amt",
                        style: {
                          color: a.amt.startsWith("−")
                            ? "var(--money-neg)"
                            : "var(--text)",
                        },
                      },
                      a.amt,
                    ),
                  ),
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement(
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
                  /*#__PURE__*/ React.createElement(DashIcon, {
                    name: "shield",
                    size: 16,
                    className: "dash-card__ic",
                  }),
                  "Responsibility split",
                ),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "dash-card__body",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "dash-split",
                  },
                  [
                    {
                      lbl: "Personal — Alex",
                      amt: "$1,612",
                      pct: 51,
                      c: "var(--owner-personal)",
                      type: "personal",
                    },
                    {
                      lbl: "Partner — Sam",
                      amt: "$888",
                      pct: 28,
                      c: "var(--owner-partner)",
                      type: "partner",
                    },
                    {
                      lbl: "Shared household",
                      amt: "$642",
                      pct: 21,
                      c: "var(--owner-shared)",
                      type: "shared",
                    },
                  ].map((r) =>
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        key: r.lbl,
                      },
                      /*#__PURE__*/ React.createElement(
                        "div",
                        {
                          className: "dash-splitrow__top",
                        },
                        /*#__PURE__*/ React.createElement(
                          "span",
                          {
                            className: "dash-splitrow__lbl",
                          },
                          /*#__PURE__*/ React.createElement("span", {
                            style: {
                              width: 9,
                              height: 9,
                              borderRadius: 3,
                              background: r.c,
                            },
                          }),
                          r.lbl,
                        ),
                        /*#__PURE__*/ React.createElement(
                          "span",
                          {
                            className: "dash-splitrow__amt",
                          },
                          r.amt,
                          " \xB7 ",
                          r.pct,
                          "%",
                        ),
                      ),
                      /*#__PURE__*/ React.createElement(
                        "div",
                        {
                          className: "dash-bar",
                        },
                        /*#__PURE__*/ React.createElement("div", {
                          className: "dash-bar__fill",
                          style: {
                            width: r.pct + "%",
                            background: r.c,
                          },
                        }),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
          /*#__PURE__*/ React.createElement(
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
                /*#__PURE__*/ React.createElement(DashIcon, {
                  name: "receipt",
                  size: 16,
                  className: "dash-card__ic",
                }),
                "Recent activity",
              ),
              /*#__PURE__*/ React.createElement(
                Badge,
                {
                  tone: "warning",
                  dot: true,
                },
                "6 need an owner",
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "dash-card__body",
                style: {
                  padding: 0,
                },
              },
              /*#__PURE__*/ React.createElement(TransactionRow, {
                date: "08 Jun",
                merchant: "Whole Foods Market",
                category: "Groceries",
                categoryColor: "var(--chart-2)",
                owner: /*#__PURE__*/ React.createElement(OwnerChip, {
                  name: "Household",
                  type: "shared",
                  bare: true,
                }),
                amount: "642.18",
                status: "needs-owner",
                confidence: "low",
              }),
              /*#__PURE__*/ React.createElement(TransactionRow, {
                date: "08 Jun",
                merchant: "Acme Payroll",
                category: "Income",
                categoryColor: "var(--chart-1)",
                owner: /*#__PURE__*/ React.createElement(OwnerChip, {
                  name: "Alex Tan",
                  type: "personal",
                  bare: true,
                }),
                amount: "6,200.00",
                positive: true,
                status: "reconciled",
              }),
              /*#__PURE__*/ React.createElement(TransactionRow, {
                date: "07 Jun",
                merchant: "Pacific Gas & Electric",
                category: "Housing",
                categoryColor: "var(--chart-1)",
                owner: /*#__PURE__*/ React.createElement(OwnerChip, {
                  name: "Household",
                  type: "shared",
                  bare: true,
                }),
                amount: "148.90",
                status: "reconciled",
              }),
              /*#__PURE__*/ React.createElement(TransactionRow, {
                date: "06 Jun",
                merchant: "Spotify",
                category: "Subscriptions",
                categoryColor: "var(--chart-4)",
                owner: /*#__PURE__*/ React.createElement(OwnerChip, {
                  name: "Sam Okafor",
                  type: "partner",
                  bare: true,
                }),
                amount: "14.99",
                status: "imported",
                confidence: "high",
              }),
            ),
          ),
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

  // ui_kits/methodology/MethodologyScreen.jsx
  try {
    (() => {
      /* Neko Finance — Methodology / insights screen. Private, source-neutral rules
   in the editorial (Newsreader) voice + derived insights. window.MethodologyApp. */
      const MET_NS = window.NekoFinanceDesignSystem_9bd1cd;
      const { Button, Badge, OwnerChip } = MET_NS;
      const MetIcon = window.Icon;
      const metCSS = `
.met{max-width:980px;margin:0 auto;display:grid;grid-template-columns:1fr 244px;gap:34px;align-items:start;}
.met-main{min-width:0;}
.met-eyebrow{font-size:11px;font-weight:700;letter-spacing:.08em;text-transform:uppercase;color:var(--primary);margin-bottom:12px;}
.met-title{font-family:var(--font-serif);font-size:34px;font-weight:500;line-height:1.12;letter-spacing:-0.01em;color:var(--text-strong);margin:0 0 12px;}
.met-lede{font-family:var(--font-serif);font-size:17px;line-height:1.6;color:var(--text-muted);max-width:60ch;}
.met-lede em{color:var(--text);font-style:italic;}
.met-private{display:inline-flex;align-items:center;gap:8px;margin-top:16px;padding:7px 12px;border-radius:var(--radius-pill);background:var(--primary-quiet);border:1px solid rgba(63,191,143,.22);font-size:12px;font-weight:600;color:var(--primary);}
.met-insights{display:grid;grid-template-columns:repeat(3,1fr);gap:12px;margin:26px 0 30px;}
.met-ins{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);padding:13px 14px;box-shadow:var(--shadow-1);}
.met-ins__v{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-size:22px;font-weight:600;color:var(--text-strong);}
.met-ins__l{font-size:11.5px;color:var(--text-muted);margin-top:4px;line-height:1.35;}
.met-rule{border-top:1px solid var(--border);padding:22px 0;}
.met-rule:first-of-type{border-top:none;}
.met-rule__num{font-family:var(--font-mono);font-size:11px;color:var(--text-faint);}
.met-rule__h{display:flex;align-items:center;gap:10px;margin:6px 0 9px;}
.met-rule__title{font-size:17px;font-weight:700;color:var(--text-strong);letter-spacing:-0.01em;}
.met-rule__body{font-family:var(--font-serif);font-size:15.5px;line-height:1.62;color:var(--text-muted);max-width:62ch;}
.met-rule__body b{color:var(--text);font-weight:600;}
.met-rule__body em{font-style:italic;color:var(--text);}
.met-eg{margin-top:13px;display:flex;align-items:stretch;gap:0;background:var(--bg-subtle);border:1px solid var(--border);border-radius:var(--radius-sm);overflow:hidden;max-width:560px;}
.met-eg__tag{display:flex;align-items:center;padding:0 11px;background:var(--surface-2);border-right:1px solid var(--border);font-size:9.5px;font-weight:700;letter-spacing:.06em;text-transform:uppercase;color:var(--text-faint);}
.met-eg__body{padding:10px 13px;display:flex;flex-direction:column;gap:5px;flex:1;}
.met-eg__line{display:flex;justify-content:space-between;gap:14px;font-size:12.5px;color:var(--text-muted);align-items:baseline;}
.met-eg__line span:first-child{white-space:nowrap;overflow:hidden;text-overflow:ellipsis;min-width:0;}
.met-eg__line span:last-child{font-family:var(--font-money);font-variant-numeric:tabular-nums;color:var(--text);flex:none;}
.met-eg__tot{border-top:1px solid var(--border);padding-top:6px;margin-top:1px;font-weight:700;color:var(--text)!important;}
.met-eg__tot span:last-child{color:var(--primary)!important;font-weight:700;}
.met-rule__foot{margin-top:13px;display:flex;align-items:center;gap:10px;}
.met-rail{position:sticky;top:0;display:flex;flex-direction:column;gap:8px;}
.met-rail__h{font-size:10.5px;font-weight:700;letter-spacing:.07em;text-transform:uppercase;color:var(--text-faint);padding:0 4px 4px;}
.met-toc{display:flex;flex-direction:column;}
.met-toc__i{display:flex;align-items:center;gap:9px;padding:7px 10px;border-radius:var(--radius-sm);font-size:12.5px;color:var(--text-muted);cursor:pointer;border:none;background:none;text-align:left;width:100%;transition:var(--t-hover);}
.met-toc__i:hover{background:var(--surface-hover);color:var(--text);}
.met-toc__i--on{background:var(--surface-selected);color:var(--text-strong);font-weight:600;}
.met-toc__n{font-family:var(--font-mono);font-size:10.5px;color:var(--text-faint);width:16px;flex:none;}
.met-note{margin-top:14px;background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);padding:13px 14px;}
.met-note__t{font-size:12px;font-weight:700;color:var(--text);display:flex;align-items:center;gap:7px;}
.met-note__d{font-size:11.5px;color:var(--text-muted);line-height:1.5;margin-top:6px;}
@media (max-width:920px){ .met{grid-template-columns:1fr;} .met-rail{position:static;} .met-insights{grid-template-columns:1fr 1fr;} }
`;
      function injectMet() {
        if (document.getElementById("met-css")) return;
        const s = document.createElement("style");
        s.id = "met-css";
        s.textContent = metCSS;
        document.head.appendChild(s);
      }
      const RULES = [
        {
          id: "split",
          title: "How shared expenses are split",
          body: /*#__PURE__*/ React.createElement(
            React.Fragment,
            null,
            "A shared expense is divided by ",
            /*#__PURE__*/ React.createElement("em", null, "responsibility"),
            ", not by who paid. The ",
            /*#__PURE__*/ React.createElement("b", null, "payer"),
            " is recorded so accounts reconcile; the ",
            /*#__PURE__*/ React.createElement("b", null, "beneficiary"),
            " decides whose budget it lands in; the ",
            /*#__PURE__*/ React.createElement("b", null, "responsible owner"),
            " is who ultimately carries it. By default, household charges split ",
            /*#__PURE__*/ React.createElement("b", null, "50 / 50"),
            " unless a line sets its own ratio.",
          ),
          eg: {
            tag: "Rent",
            lines: [
              {
                l: "Paid by Alex",
                v: "2,150.00",
              },
              {
                l: "Alex's share (50%)",
                v: "1,075.00",
              },
              {
                l: "Sam owes Alex",
                v: "1,075.00",
              },
            ],
            tot: {
              l: "Household total",
              v: "$2,150.00",
            },
          },
        },
        {
          id: "income",
          title: "What counts as income",
          body: /*#__PURE__*/ React.createElement(
            React.Fragment,
            null,
            "Only ",
            /*#__PURE__*/ React.createElement("b", null, "realized inflows"),
            " to an owned account count as income \u2014 salary, interest, reimbursements received. Internal ",
            /*#__PURE__*/ React.createElement("em", null, "transfers"),
            " between your own accounts are netted to zero so they never inflate cashflow, and a reimbursement is matched back to the expense it offsets rather than counted twice.",
          ),
          eg: {
            tag: "June",
            lines: [
              {
                l: "Salary",
                v: "6,200.00",
              },
              {
                l: "Transfer in (own)",
                v: "0.00",
              },
              {
                l: "Reimbursement",
                v: "48.00",
              },
            ],
            tot: {
              l: "Counted income",
              v: "$6,248.00",
            },
          },
        },
        {
          id: "savings",
          title: "How the savings rate is measured",
          body: /*#__PURE__*/ React.createElement(
            React.Fragment,
            null,
            "Savings rate is ",
            /*#__PURE__*/ React.createElement("b", null, "net saved \xF7 net income"),
            " over the period, where net saved is income minus all spending including shared responsibility. It is a ",
            /*#__PURE__*/ React.createElement("em", null, "source-neutral"),
            " definition \u2014 it does not assume any particular budgeting framework, only your own categorized rows.",
          ),
          eg: {
            tag: "June",
            lines: [
              {
                l: "Net income",
                v: "6,248.00",
              },
              {
                l: "Total spending",
                v: "4,120.00",
              },
            ],
            tot: {
              l: "Savings rate",
              v: "34%",
            },
          },
        },
        {
          id: "confidence",
          title: "When Mia asks before classifying",
          body: /*#__PURE__*/ React.createElement(
            React.Fragment,
            null,
            "Every imported row gets a category and owner with a ",
            /*#__PURE__*/ React.createElement("b", null, "confidence"),
            " score. High-confidence matches apply silently; ",
            /*#__PURE__*/ React.createElement("b", null, "medium and low"),
            " ones are flagged for your review and never written back to the sheet until you confirm. Confidence comes from your ",
            /*#__PURE__*/ React.createElement("em", null, "own"),
            " prior decisions, not an external dataset.",
          ),
          eg: {
            tag: "Rule",
            lines: [
              {
                l: "Merchant match",
                v: "high",
              },
              {
                l: "New merchant",
                v: "low",
              },
              {
                l: "Asks before write",
                v: "yes",
              },
            ],
            tot: {
              l: "Rows flagged · June",
              v: "6",
            },
          },
        },
      ];
      function MethodologyApp() {
        injectMet();
        const [nav, setNav] = React.useState("methodology");
        const [active, setActive] = React.useState("split");
        const refs = React.useRef({});
        const go = (id) => {
          setActive(id);
          const el = refs.current[id];
          const body = el && el.closest(".ak-body");
          if (el && body) {
            body.scrollTo({
              top: el.offsetTop - 16,
              behavior: "smooth",
            });
          }
        };
        return /*#__PURE__*/ React.createElement(
          window.AppShell,
          {
            active: nav,
            onNav: (k) => (window.__nekoRoute ? window.__nekoRoute(k) : setNav(k)),
            title: "Methodology",
            crumb: "Private rules \xB7 this ledger",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "met",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "met-main",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "met-eyebrow",
                },
                "Methodology",
              ),
              /*#__PURE__*/ React.createElement(
                "h1",
                {
                  className: "met-title",
                },
                "The rules behind every number",
              ),
              /*#__PURE__*/ React.createElement(
                "p",
                {
                  className: "met-lede",
                },
                "Neko explains your money with rules you can read and change. They are ",
                /*#__PURE__*/ React.createElement(
                  "em",
                  null,
                  "private and source-neutral",
                ),
                " \u2014 derived from how you categorize your own ledger, never from a public course or a shared model.",
              ),
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "met-private",
                },
                /*#__PURE__*/ React.createElement(MetIcon, {
                  name: "lock",
                  size: 13,
                }),
                "Private to this ledger \xB7 editable",
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "met-insights",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "met-ins",
                  },
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "met-ins__v",
                    },
                    "$642",
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "met-ins__l",
                    },
                    "Shared this month, split by responsibility",
                  ),
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "met-ins",
                  },
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "met-ins__v",
                    },
                    "34%",
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "met-ins__l",
                    },
                    "Savings rate, by your definition",
                  ),
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "met-ins",
                  },
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "met-ins__v",
                    },
                    "6",
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "met-ins__l",
                    },
                    "Rows held for your review",
                  ),
                ),
              ),
              RULES.map((r, i) =>
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "met-rule",
                    key: r.id,
                    ref: (el) => (refs.current[r.id] = el),
                  },
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "met-rule__num",
                    },
                    "Rule ",
                    String(i + 1).padStart(2, "0"),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "met-rule__h",
                    },
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "met-rule__title",
                      },
                      r.title,
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "met-rule__body",
                    },
                    r.body,
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "met-eg",
                    },
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "met-eg__tag",
                      },
                      r.eg.tag,
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "met-eg__body",
                      },
                      r.eg.lines.map((l, j) =>
                        /*#__PURE__*/ React.createElement(
                          "div",
                          {
                            className: "met-eg__line",
                            key: j,
                          },
                          /*#__PURE__*/ React.createElement("span", null, l.l),
                          /*#__PURE__*/ React.createElement("span", null, l.v),
                        ),
                      ),
                      /*#__PURE__*/ React.createElement(
                        "div",
                        {
                          className: "met-eg__line met-eg__tot",
                        },
                        /*#__PURE__*/ React.createElement("span", null, r.eg.tot.l),
                        /*#__PURE__*/ React.createElement("span", null, r.eg.tot.v),
                      ),
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "met-rule__foot",
                    },
                    /*#__PURE__*/ React.createElement(
                      Button,
                      {
                        variant: "ghost",
                        size: "sm",
                        iconLeft: /*#__PURE__*/ React.createElement(MetIcon, {
                          name: "pencil",
                          size: 14,
                        }),
                      },
                      "Edit rule",
                    ),
                    /*#__PURE__*/ React.createElement(
                      Badge,
                      {
                        tone: "neutral",
                      },
                      "Applied automatically",
                    ),
                  ),
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "aside",
              {
                className: "met-rail",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "met-rail__h",
                },
                "On this page",
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "met-toc",
                },
                RULES.map((r, i) =>
                  /*#__PURE__*/ React.createElement(
                    "button",
                    {
                      key: r.id,
                      className:
                        "met-toc__i" + (active === r.id ? " met-toc__i--on" : ""),
                      onClick: () => go(r.id),
                    },
                    /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        className: "met-toc__n",
                      },
                      String(i + 1).padStart(2, "0"),
                    ),
                    r.title,
                  ),
                ),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "met-note",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "met-note__t",
                  },
                  /*#__PURE__*/ React.createElement(MetIcon, {
                    name: "shield",
                    size: 14,
                    style: {
                      color: "var(--primary)",
                    },
                  }),
                  "Source-neutral by design",
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "met-note__d",
                  },
                  "These rules reference only your ledger. Neko never cites a public methodology or sends your rules anywhere.",
                ),
              ),
            ),
          ),
        );
      }
      window.MethodologyApp = MethodologyApp;
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
      /* Neko Finance — Settings / privacy screen. Local data, Google OAuth,
   AI provider keys, people/ownership, update channel. window.SettingsApp. */
      const SET_NS = window.NekoFinanceDesignSystem_9bd1cd;
      const { Switch, SegmentedControl, Input, Button, Badge, OwnerChip } = SET_NS;
      const SetIcon = window.Icon;
      const setCSS = `
.set{max-width:760px;margin:0 auto;display:flex;flex-direction:column;gap:28px;}
.set-sec__head{margin-bottom:11px;}
.set-sec__title{font-size:15px;font-weight:700;color:var(--text-strong);letter-spacing:-0.005em;display:flex;align-items:center;gap:9px;}
.set-sec__ic{color:var(--text-faint);}
.set-sec__sub{font-size:12.5px;color:var(--text-muted);margin-top:3px;margin-left:25px;}
.set-panel{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-md);box-shadow:var(--shadow-1);overflow:hidden;}
.set-row{display:flex;align-items:center;gap:14px;padding:14px 16px;border-bottom:1px solid var(--border);}
.set-row:last-child{border-bottom:none;}
.set-row__main{flex:1;min-width:0;}
.set-row__t{font-size:13.5px;font-weight:600;color:var(--text);}
.set-row__d{font-size:12px;color:var(--text-muted);margin-top:2px;line-height:1.4;}
.set-row__d code{font-family:var(--font-mono);font-size:11px;background:var(--surface-2);padding:1px 5px;border-radius:4px;color:var(--text);}
.set-row__ctl{flex:none;display:flex;align-items:center;gap:8px;}
.set-conn{display:flex;align-items:center;gap:12px;padding:15px 16px;background:var(--bg-subtle);border-bottom:1px solid var(--border);}
.set-conn__logo{width:38px;height:38px;border-radius:10px;background:var(--surface);border:1px solid var(--border);display:flex;align-items:center;justify-content:center;color:var(--success-500);flex:none;}
.set-conn__t{font-size:14px;font-weight:700;color:var(--text-strong);}
.set-conn__s{font-size:12px;color:var(--text-muted);margin-top:2px;display:flex;align-items:center;gap:6px;}
.set-key{display:flex;align-items:center;gap:8px;background:var(--surface-2);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0 10px;height:34px;font-family:var(--font-mono);font-size:12.5px;color:var(--text-muted);}
.set-people{display:flex;flex-direction:column;}
.set-danger{border-color:rgba(203,70,62,.28);}
[data-theme="light"] .set-danger,.set-danger{border-color:color-mix(in srgb,var(--danger-500) 30%,var(--border));}
.set-danger .set-row__t{color:var(--danger-500);}
.set-meta{display:flex;align-items:center;gap:8px;font-size:11.5px;color:var(--text-faint);font-family:var(--font-mono);}
`;
      function injectSet() {
        if (document.getElementById("set-css")) return;
        const s = document.createElement("style");
        s.id = "set-css";
        s.textContent = setCSS;
        document.head.appendChild(s);
      }
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
              "div",
              {
                className: "set-sec__title",
              },
              /*#__PURE__*/ React.createElement(SetIcon, {
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
      function SettingsApp() {
        injectSet();
        const [nav, setNav] = React.useState("settings");
        const [approve, setApprove] = React.useState(true);
        const [autoCat, setAutoCat] = React.useState(true);
        const [telemetry, setTelemetry] = React.useState(false);
        const [provider, setProvider] = React.useState("local");
        const [channel, setChannel] = React.useState("stable");
        const [revealKey, setRevealKey] = React.useState(false);
        return /*#__PURE__*/ React.createElement(
          window.AppShell,
          {
            active: nav,
            onNav: (k) => (window.__nekoRoute ? window.__nekoRoute(k) : setNav(k)),
            title: "Settings & privacy",
            crumb: "Local \xB7 this device",
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "set",
            },
            /*#__PURE__*/ React.createElement(
              Section,
              {
                icon: "link",
                title: "Connection",
                sub: "Neko reads your Google Sheet. It never writes without your approval.",
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
                    /*#__PURE__*/ React.createElement(SetIcon, {
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
                        style: {
                          width: 6,
                          height: 6,
                          borderRadius: "50%",
                          background: "var(--success-500)",
                          display: "inline-block",
                        },
                      }),
                      "conta-google-conectada \xB7 read-only scope",
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    Badge,
                    {
                      tone: "success",
                      dot: true,
                    },
                    "Connected",
                  ),
                  /*#__PURE__*/ React.createElement(
                    Button,
                    {
                      variant: "secondary",
                      size: "sm",
                    },
                    "Reconnect",
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
                      "Active sheet",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__d",
                      },
                      "Workbook ",
                      /*#__PURE__*/ React.createElement("code", null, "Expenses 2025"),
                      " \xB7 248 rows \xB7 synced 2 min ago",
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
                        iconLeft: /*#__PURE__*/ React.createElement(SetIcon, {
                          name: "refresh",
                          size: 14,
                        }),
                      },
                      "Re-sync",
                    ),
                    /*#__PURE__*/ React.createElement(
                      Button,
                      {
                        variant: "ghost",
                        size: "sm",
                      },
                      "Change",
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
                      "Write access",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__d",
                      },
                      "Neko proposes edits as a diff. Nothing is written until you approve.",
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
                      "Approval required",
                    ),
                  ),
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement(
              Section,
              {
                icon: "sparkles",
                title: "AI copilot (Mia)",
                sub: "Choose where Mia's model runs. Local keeps everything on this device.",
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
                      "Provider",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__d",
                      },
                      provider === "local"
                        ? "On-device model — no data leaves your machine."
                        : "Calls an external API. Your sheet rows are sent to the provider.",
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "set-row__ctl",
                    },
                    /*#__PURE__*/ React.createElement(SegmentedControl, {
                      value: provider,
                      onChange: setProvider,
                      options: [
                        {
                          value: "local",
                          label: "Local",
                        },
                        {
                          value: "api",
                          label: "API key",
                        },
                      ],
                    }),
                  ),
                ),
                provider === "api"
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
                          "API key",
                        ),
                        /*#__PURE__*/ React.createElement(
                          "div",
                          {
                            className: "set-row__d",
                          },
                          "Stored encrypted in your local keychain \u2014 never synced.",
                        ),
                      ),
                      /*#__PURE__*/ React.createElement(
                        "div",
                        {
                          className: "set-row__ctl",
                        },
                        /*#__PURE__*/ React.createElement(
                          "span",
                          {
                            className: "set-key",
                          },
                          revealKey ? "sk-neko-7f3a9c21b8e4" : "sk-neko-••••••••••••",
                        ),
                        /*#__PURE__*/ React.createElement(
                          Button,
                          {
                            variant: "ghost",
                            size: "sm",
                            onClick: () => setRevealKey((v) => !v),
                          },
                          revealKey ? "Hide" : "Reveal",
                        ),
                      ),
                    )
                  : null,
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
                      "Auto-categorize on import",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__d",
                      },
                      "Mia suggests a category & owner for each new row. You confirm low-confidence ones.",
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "set-row__ctl",
                    },
                    /*#__PURE__*/ React.createElement(Switch, {
                      checked: autoCat,
                      onChange: setAutoCat,
                    }),
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
                      "Require approval for sheet writes",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__d",
                      },
                      "Strongly recommended. Disabling lets Mia write approved rule-matches directly.",
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "set-row__ctl",
                    },
                    /*#__PURE__*/ React.createElement(Switch, {
                      checked: approve,
                      onChange: setApprove,
                    }),
                  ),
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement(
              Section,
              {
                icon: "shield",
                title: "Privacy & data",
                sub: "Neko is local-first. There is no Neko account and no backend.",
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
                      "Data location",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__d",
                      },
                      "Encrypted SQLite at ",
                      /*#__PURE__*/ React.createElement(
                        "code",
                        null,
                        "~/Library/Neko/neko.db",
                      ),
                      " on this device only.",
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
                        iconLeft: /*#__PURE__*/ React.createElement(SetIcon, {
                          name: "download",
                          size: 14,
                        }),
                      },
                      "Export",
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
                      "Anonymous diagnostics",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__d",
                      },
                      "Off by default. Neko sends no usage data unless you opt in.",
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "set-row__ctl",
                    },
                    /*#__PURE__*/ React.createElement(Switch, {
                      checked: telemetry,
                      onChange: setTelemetry,
                    }),
                  ),
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement(
              Section,
              {
                icon: "settings",
                title: "People & ownership",
                sub: "Who shares this ledger, and the default owner for new shared expenses.",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-panel set-people",
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
                    /*#__PURE__*/ React.createElement(OwnerChip, {
                      name: "Alex Tan",
                      type: "personal",
                      role: "You",
                    }),
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
                      "Owner",
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
                    /*#__PURE__*/ React.createElement(OwnerChip, {
                      name: "Sam Okafor",
                      type: "partner",
                      role: "Partner",
                    }),
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
                      "Can view & assign",
                    ),
                    /*#__PURE__*/ React.createElement(
                      Button,
                      {
                        variant: "ghost",
                        size: "sm",
                      },
                      "Manage",
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
                      "Default owner for shared expenses",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__d",
                      },
                      "Applied when a new charge matches a shared-venue rule.",
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "set-row__ctl",
                    },
                    /*#__PURE__*/ React.createElement(SegmentedControl, {
                      value: "shared",
                      onChange: () => {},
                      options: [
                        {
                          value: "personal",
                          label: "Personal",
                          dot: "var(--owner-personal)",
                        },
                        {
                          value: "shared",
                          label: "Household",
                          dot: "var(--owner-shared)",
                        },
                      ],
                    }),
                  ),
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement(
              Section,
              {
                icon: "refresh",
                title: "Updates",
                sub: "Neko 0.4.2 \xB7 Tauri desktop build.",
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
                      "Update channel",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__d",
                      },
                      "Stable ships monthly. Beta gets new copilot tools earlier.",
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "set-row__ctl",
                    },
                    /*#__PURE__*/ React.createElement(SegmentedControl, {
                      value: channel,
                      onChange: setChannel,
                      options: [
                        {
                          value: "stable",
                          label: "Stable",
                        },
                        {
                          value: "beta",
                          label: "Beta",
                        },
                      ],
                    }),
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
                      "Current version",
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
                        /*#__PURE__*/ React.createElement(SetIcon, {
                          name: "check",
                          size: 13,
                          style: {
                            color: "var(--success-500)",
                          },
                        }),
                        "v0.4.2 \xB7 up to date \xB7 checked today",
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
                        variant: "secondary",
                        size: "sm",
                      },
                      "Check for updates",
                    ),
                  ),
                ),
              ),
            ),
            /*#__PURE__*/ React.createElement(
              Section,
              {
                icon: "alertTriangle",
                title: "Danger zone",
                sub: "These actions affect only this device's local data.",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "set-panel set-danger",
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
                      "Disconnect Google Sheets",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__d",
                      },
                      "Removes the OAuth token. Your local data stays.",
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
                      },
                      "Disconnect",
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
                      "Erase local data",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "set-row__d",
                      },
                      "Permanently deletes the local database and cached rules.",
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
                        variant: "danger",
                        size: "sm",
                      },
                      "Erase\u2026",
                    ),
                  ),
                ),
              ),
            ),
          ),
        );
      }
      window.SettingsApp = SettingsApp;
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

  // ui_kits/transactions/TransactionsScreen.jsx
  try {
    (() => {
      /* Neko Finance — Transactions / import review. Master table + detail panel +
   Google Sheets column mapping with AI confidence. Exposes window.TransactionsApp. */
      const TX_NS = window.NekoFinanceDesignSystem_9bd1cd;
      const { TransactionRow, OwnerChip, Badge, SegmentedControl, Button, Input } =
        TX_NS;
      const TxIcon = window.Icon;
      const txCSS = `
.tx{display:flex;flex-direction:column;gap:14px;}
.tx-banner{display:flex;align-items:center;gap:13px;padding:13px 16px;background:var(--info-tint);
  border:1px solid rgba(79,166,206,.25);border-radius:var(--radius-md);}
.tx-banner__ic{width:32px;height:32px;border-radius:9px;background:var(--surface);color:var(--info-400);
  display:flex;align-items:center;justify-content:center;flex:none;}
.tx-banner__t{font-size:13.5px;font-weight:600;color:var(--text);}
.tx-banner__s{font-size:12px;color:var(--text-muted);margin-top:1px;}
.tx-banner__s b{color:var(--warning-400);font-weight:600;}
.tx-tools{display:flex;align-items:center;gap:10px;}
.tx-tools__sp{flex:1;}
.tx-grid{display:grid;grid-template-columns:1fr 384px;gap:14px;align-items:start;}
.tx-tablewrap{border:1px solid var(--border);border-radius:var(--radius-md);overflow:hidden;background:var(--surface);}
.tx-thead{display:grid;grid-template-columns:84px minmax(0,1fr) auto auto 132px;gap:14px;padding:9px 14px;
  border-bottom:1px solid var(--border);background:var(--bg-subtle);font-size:10.5px;font-weight:700;
  letter-spacing:.06em;text-transform:uppercase;color:var(--text-faint);}
.tx-thead span:last-child{text-align:right;}
.tx-thead span:nth-child(3),.tx-thead span:nth-child(4){text-align:right;}
/* detail panel */
.tx-detail{border:1px solid var(--border);border-radius:var(--radius-md);background:var(--surface);
  box-shadow:var(--shadow-1);position:sticky;top:0;overflow:hidden;}
.tx-d__head{padding:15px 16px;border-bottom:1px solid var(--border);}
.tx-d__merchant{font-size:16px;font-weight:700;color:var(--text-strong);letter-spacing:-0.01em;}
.tx-d__meta{display:flex;align-items:center;gap:8px;margin-top:6px;}
.tx-d__amt{font-family:var(--font-money);font-variant-numeric:tabular-nums;font-size:26px;font-weight:600;
  color:var(--text-strong);margin-top:10px;}
.tx-d__src{font-family:var(--font-money);font-size:10.5px;color:var(--text-faint);margin-top:7px;display:flex;align-items:center;gap:6px;}
.tx-d__body{padding:14px 16px;display:flex;flex-direction:column;gap:16px;}
.tx-field__lbl{font-size:11px;font-weight:700;letter-spacing:.05em;text-transform:uppercase;color:var(--text-faint);
  margin-bottom:8px;display:flex;align-items:center;justify-content:space-between;}
.tx-sugg{display:inline-flex;align-items:center;gap:5px;font-size:10.5px;font-weight:600;color:var(--primary);}
.tx-opts{display:flex;flex-wrap:wrap;gap:7px;}
.tx-opt{display:inline-flex;align-items:center;gap:7px;padding:7px 11px;border-radius:var(--radius-sm);
  border:1px solid var(--border);background:var(--surface-elevated);font-size:12.5px;font-weight:600;color:var(--text-muted);
  cursor:pointer;transition:var(--t-hover);}
.tx-opt:hover{border-color:var(--border-strong);color:var(--text);}
.tx-opt--on{border-color:var(--primary);background:var(--primary-quiet);color:var(--text-strong);}
.tx-opt__dot{width:8px;height:8px;border-radius:50%;}
.tx-roles{display:flex;flex-direction:column;gap:8px;}
.tx-role{display:flex;align-items:center;justify-content:space-between;gap:10px;}
.tx-role__k{font-size:12px;color:var(--text-muted);}
.tx-map{border-top:1px solid var(--border);}
.tx-map__row{display:grid;grid-template-columns:1fr auto 1fr auto;gap:10px;align-items:center;padding:8px 16px;
  font-size:12px;border-bottom:1px solid var(--border);}
.tx-map__row:last-child{border-bottom:none;}
.tx-map__col{font-family:var(--font-money);color:var(--text-muted);}
.tx-map__arrow{color:var(--text-faint);}
.tx-map__field{font-weight:600;color:var(--text);}
.tx-d__foot{display:flex;gap:8px;padding:14px 16px;border-top:1px solid var(--border);background:var(--bg-subtle);}
.tx-map__head{padding:12px 16px 6px;font-size:11px;font-weight:700;letter-spacing:.05em;text-transform:uppercase;color:var(--text-faint);}
@media (max-width:1180px){
  .tx-grid{grid-template-columns:1fr;}
  .tx-detail{position:static;}
}
`;
      function injectTx() {
        if (document.getElementById("tx-css")) return;
        const s = document.createElement("style");
        s.id = "tx-css";
        s.textContent = txCSS;
        document.head.appendChild(s);
      }
      const TX_DATA = [
        {
          id: 1,
          date: "08 Jun",
          merchant: "Whole Foods Market",
          cat: "Groceries",
          catC: "var(--chart-2)",
          ownerN: "Household",
          ownerT: "shared",
          amt: "642.18",
          status: "needs-owner",
          conf: "low",
          raw: "WHOLEFDS #1042 SF CA",
        },
        {
          id: 2,
          date: "08 Jun",
          merchant: "Acme Payroll",
          cat: "Income",
          catC: "var(--chart-1)",
          ownerN: "Alex Tan",
          ownerT: "personal",
          amt: "6,200.00",
          pos: true,
          status: "reconciled",
          conf: "high",
          raw: "ACME CORP DIR DEP",
        },
        {
          id: 3,
          date: "07 Jun",
          merchant: "Pacific Gas & Electric",
          cat: "Housing",
          catC: "var(--chart-1)",
          ownerN: "Household",
          ownerT: "shared",
          amt: "148.90",
          status: "reconciled",
          conf: "high",
          raw: "PG&E AUTOPAY",
        },
        {
          id: 4,
          date: "07 Jun",
          merchant: "Blue Bottle Coffee",
          cat: "Dining",
          catC: "var(--chart-5)",
          ownerN: "Sam Okafor",
          ownerT: "partner",
          amt: "9.50",
          status: "needs-owner",
          conf: "medium",
          raw: "SQ *BLUE BOTTLE",
        },
        {
          id: 5,
          date: "06 Jun",
          merchant: "Spotify",
          cat: "Subscriptions",
          catC: "var(--chart-4)",
          ownerN: "Sam Okafor",
          ownerT: "partner",
          amt: "14.99",
          status: "imported",
          conf: "high",
          raw: "SPOTIFY P0A1B2",
        },
        {
          id: 6,
          date: "06 Jun",
          merchant: "Shell",
          cat: "Transport",
          catC: "var(--chart-3)",
          ownerN: "—",
          ownerT: "personal",
          amt: "58.20",
          status: "needs-owner",
          conf: "low",
          raw: "SHELL OIL 5731",
        },
      ];
      const CATS = [
        {
          n: "Groceries",
          c: "var(--chart-2)",
        },
        {
          n: "Dining",
          c: "var(--chart-5)",
        },
        {
          n: "Housing",
          c: "var(--chart-1)",
        },
        {
          n: "Transport",
          c: "var(--chart-3)",
        },
        {
          n: "Subscriptions",
          c: "var(--chart-4)",
        },
      ];
      function TransactionsApp() {
        injectTx();
        const [nav, setNav] = React.useState("transactions");
        const [selId, setSelId] = React.useState(1);
        const [scope, setScope] = React.useState("all");
        const sel = TX_DATA.find((t) => t.id === selId);
        const [cat, setCat] = React.useState(sel.cat);
        const [ownerType, setOwnerType] = React.useState(sel.ownerT);
        React.useEffect(() => {
          setCat(sel.cat);
          setOwnerType(sel.ownerT);
        }, [selId]);
        const rows = TX_DATA.filter((t) => scope === "all" || t.ownerT === scope);
        const right = React.createElement(
          Button,
          {
            variant: "secondary",
            size: "sm",
            iconLeft: React.createElement(TxIcon, {
              name: "refresh",
              size: 15,
            }),
          },
          "Re-sync sheet",
        );
        return /*#__PURE__*/ React.createElement(
          window.AppShell,
          {
            active: nav,
            onNav: (k) => (window.__nekoRoute ? window.__nekoRoute(k) : setNav(k)),
            title: "Transactions",
            crumb: "Import review \xB7 Expenses 2025",
            right: right,
          },
          /*#__PURE__*/ React.createElement(
            "div",
            {
              className: "tx",
            },
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "tx-banner",
              },
              /*#__PURE__*/ React.createElement(
                "span",
                {
                  className: "tx-banner__ic",
                },
                /*#__PURE__*/ React.createElement(TxIcon, {
                  name: "table",
                  size: 17,
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
                    className: "tx-banner__t",
                  },
                  "Imported 248 rows from \u201CExpenses 2025 \u2192 Aug\u201D",
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "tx-banner__s",
                  },
                  /*#__PURE__*/ React.createElement("b", null, "6 need an owner"),
                  " \xB7 12 low-confidence categories \xB7 mapped 8 of 8 columns",
                ),
              ),
              /*#__PURE__*/ React.createElement(
                Button,
                {
                  variant: "ghost",
                  size: "sm",
                },
                "Review mapping",
              ),
              /*#__PURE__*/ React.createElement(
                Button,
                {
                  variant: "primary",
                  size: "sm",
                  iconLeft: /*#__PURE__*/ React.createElement(TxIcon, {
                    name: "check",
                    size: 15,
                  }),
                },
                "Confirm all clean",
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "tx-tools",
              },
              /*#__PURE__*/ React.createElement(SegmentedControl, {
                value: scope,
                onChange: setScope,
                options: [
                  {
                    value: "all",
                    label: "All",
                  },
                  {
                    value: "personal",
                    label: "Personal",
                    dot: "var(--owner-personal)",
                  },
                  {
                    value: "partner",
                    label: "Partner",
                    dot: "var(--owner-partner)",
                  },
                  {
                    value: "shared",
                    label: "Shared",
                    dot: "var(--owner-shared)",
                  },
                ],
              }),
              /*#__PURE__*/ React.createElement(
                Badge,
                {
                  tone: "neutral",
                },
                rows.length,
                " shown",
              ),
              /*#__PURE__*/ React.createElement("span", {
                className: "tx-tools__sp",
              }),
              /*#__PURE__*/ React.createElement(
                Button,
                {
                  variant: "ghost",
                  size: "sm",
                  iconLeft: /*#__PURE__*/ React.createElement(TxIcon, {
                    name: "filter",
                    size: 15,
                  }),
                },
                "Confidence",
              ),
            ),
            /*#__PURE__*/ React.createElement(
              "div",
              {
                className: "tx-grid",
              },
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "tx-tablewrap",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "tx-thead",
                  },
                  /*#__PURE__*/ React.createElement("span", null, "Date"),
                  /*#__PURE__*/ React.createElement("span", null, "Merchant"),
                  /*#__PURE__*/ React.createElement("span", null, "Owner"),
                  /*#__PURE__*/ React.createElement("span", null, "Status"),
                  /*#__PURE__*/ React.createElement("span", null, "Amount"),
                ),
                rows.map((t) =>
                  /*#__PURE__*/ React.createElement(TransactionRow, {
                    key: t.id,
                    date: t.date,
                    merchant: t.merchant,
                    category: t.cat,
                    categoryColor: t.catC,
                    owner: /*#__PURE__*/ React.createElement(OwnerChip, {
                      name: t.ownerN === "—" ? "Unassigned" : t.ownerN,
                      type: t.ownerT,
                      bare: true,
                    }),
                    amount: t.amt,
                    positive: t.pos,
                    status: t.status,
                    confidence: t.conf,
                    selected: t.id === selId,
                    onClick: () => setSelId(t.id),
                  }),
                ),
              ),
              /*#__PURE__*/ React.createElement(
                "div",
                {
                  className: "tx-detail",
                },
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "tx-d__head",
                  },
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "tx-d__merchant",
                    },
                    sel.merchant,
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "tx-d__meta",
                    },
                    /*#__PURE__*/ React.createElement(
                      Badge,
                      {
                        tone:
                          sel.status === "reconciled"
                            ? "success"
                            : sel.status === "imported"
                              ? "info"
                              : "warning",
                        dot: true,
                      },
                      sel.status === "needs-owner"
                        ? "Needs owner"
                        : sel.status === "imported"
                          ? "Imported"
                          : "Reconciled",
                    ),
                    /*#__PURE__*/ React.createElement(
                      "span",
                      {
                        style: {
                          fontSize: 11.5,
                          color: "var(--text-faint)",
                        },
                      },
                      sel.date,
                      " \xB7 2025",
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "tx-d__amt",
                      style: {
                        color: sel.pos ? "var(--money-pos)" : "var(--text-strong)",
                      },
                    },
                    sel.pos ? "+ " : "− ",
                    "$",
                    sel.amt,
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "tx-d__src",
                    },
                    /*#__PURE__*/ React.createElement(TxIcon, {
                      name: "table",
                      size: 12,
                    }),
                    "Sheet \u2018Expenses 2025\u2019 \xB7 row 1,204 \xB7 \u201C",
                    sel.raw,
                    "\u201D",
                  ),
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "tx-d__body",
                  },
                  /*#__PURE__*/ React.createElement(
                    "div",
                    null,
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "tx-field__lbl",
                      },
                      /*#__PURE__*/ React.createElement("span", null, "Category"),
                      /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          className: "tx-sugg",
                        },
                        /*#__PURE__*/ React.createElement(TxIcon, {
                          name: "sparkles",
                          size: 12,
                        }),
                        "Mia: ",
                        sel.cat,
                        " (",
                        sel.conf,
                        ")",
                      ),
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "tx-opts",
                      },
                      CATS.map((c) =>
                        /*#__PURE__*/ React.createElement(
                          "button",
                          {
                            key: c.n,
                            className: "tx-opt" + (cat === c.n ? " tx-opt--on" : ""),
                            onClick: () => setCat(c.n),
                          },
                          /*#__PURE__*/ React.createElement("span", {
                            className: "tx-opt__dot",
                            style: {
                              background: c.c,
                            },
                          }),
                          c.n,
                        ),
                      ),
                    ),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    null,
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "tx-field__lbl",
                      },
                      /*#__PURE__*/ React.createElement("span", null, "Ownership"),
                    ),
                    /*#__PURE__*/ React.createElement(SegmentedControl, {
                      value: ownerType,
                      onChange: setOwnerType,
                      options: [
                        {
                          value: "personal",
                          label: "Personal",
                          dot: "var(--owner-personal)",
                        },
                        {
                          value: "partner",
                          label: "Partner",
                          dot: "var(--owner-partner)",
                        },
                        {
                          value: "shared",
                          label: "Shared",
                          dot: "var(--owner-shared)",
                        },
                      ],
                    }),
                  ),
                  /*#__PURE__*/ React.createElement(
                    "div",
                    null,
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "tx-field__lbl",
                      },
                      /*#__PURE__*/ React.createElement("span", null, "Roles"),
                    ),
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "tx-roles",
                      },
                      /*#__PURE__*/ React.createElement(
                        "div",
                        {
                          className: "tx-role",
                        },
                        /*#__PURE__*/ React.createElement(
                          "span",
                          {
                            className: "tx-role__k",
                          },
                          "Payer",
                        ),
                        /*#__PURE__*/ React.createElement(OwnerChip, {
                          name: "Sam Okafor",
                          type: "partner",
                          bare: true,
                        }),
                      ),
                      /*#__PURE__*/ React.createElement(
                        "div",
                        {
                          className: "tx-role",
                        },
                        /*#__PURE__*/ React.createElement(
                          "span",
                          {
                            className: "tx-role__k",
                          },
                          "Beneficiary",
                        ),
                        /*#__PURE__*/ React.createElement(OwnerChip, {
                          name: ownerType === "shared" ? "Household" : "Alex Tan",
                          type: ownerType === "shared" ? "shared" : "personal",
                          bare: true,
                        }),
                      ),
                      /*#__PURE__*/ React.createElement(
                        "div",
                        {
                          className: "tx-role",
                        },
                        /*#__PURE__*/ React.createElement(
                          "span",
                          {
                            className: "tx-role__k",
                          },
                          "Responsible",
                        ),
                        /*#__PURE__*/ React.createElement(OwnerChip, {
                          name: ownerType === "shared" ? "Household" : "Alex Tan",
                          type: ownerType,
                          bare: true,
                        }),
                      ),
                    ),
                  ),
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "tx-map",
                  },
                  /*#__PURE__*/ React.createElement(
                    "div",
                    {
                      className: "tx-map__head",
                    },
                    "Sheet column mapping",
                  ),
                  [
                    {
                      col: "Col B · Date",
                      field: "date",
                      conf: "high",
                    },
                    {
                      col: "Col C · Description",
                      field: "merchant",
                      conf: "high",
                    },
                    {
                      col: "Col D · Amount",
                      field: "amount",
                      conf: "high",
                    },
                    {
                      col: "Col F · Tag",
                      field: "category",
                      conf: "low",
                    },
                  ].map((m) =>
                    /*#__PURE__*/ React.createElement(
                      "div",
                      {
                        className: "tx-map__row",
                        key: m.field,
                      },
                      /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          className: "tx-map__col",
                        },
                        m.col,
                      ),
                      /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          className: "tx-map__arrow",
                        },
                        "\u2192",
                      ),
                      /*#__PURE__*/ React.createElement(
                        "span",
                        {
                          className: "tx-map__field",
                        },
                        m.field,
                      ),
                      /*#__PURE__*/ React.createElement(
                        Badge,
                        {
                          tone: m.conf === "high" ? "success" : "warning",
                        },
                        m.conf,
                      ),
                    ),
                  ),
                ),
                /*#__PURE__*/ React.createElement(
                  "div",
                  {
                    className: "tx-d__foot",
                  },
                  /*#__PURE__*/ React.createElement(
                    Button,
                    {
                      variant: "primary",
                      size: "sm",
                      fullWidth: true,
                      iconLeft: /*#__PURE__*/ React.createElement(TxIcon, {
                        name: "check",
                        size: 15,
                      }),
                    },
                    "Confirm & next",
                  ),
                  /*#__PURE__*/ React.createElement(
                    Button,
                    {
                      variant: "ghost",
                      size: "sm",
                    },
                    "Skip",
                  ),
                ),
              ),
            ),
          ),
        );
      }
      window.TransactionsApp = TransactionsApp;
    })();
  } catch (e) {
    __ds_ns.__errors.push({
      path: "ui_kits/transactions/TransactionsScreen.jsx",
      error: String((e && e.message) || e),
    });
  }

  __ds_ns.ChatBubble = __ds_scope.ChatBubble;

  __ds_ns.Citation = __ds_scope.Citation;

  __ds_ns.EmptyState = __ds_scope.EmptyState;

  __ds_ns.Badge = __ds_scope.Badge;

  __ds_ns.Button = __ds_scope.Button;

  __ds_ns.Input = __ds_scope.Input;

  __ds_ns.SegmentedControl = __ds_scope.SegmentedControl;

  __ds_ns.Switch = __ds_scope.Switch;

  __ds_ns.ApprovalDiffCard = __ds_scope.ApprovalDiffCard;

  __ds_ns.HealthBadge = __ds_scope.HealthBadge;

  __ds_ns.MetricTile = __ds_scope.MetricTile;

  __ds_ns.OwnerChip = __ds_scope.OwnerChip;

  __ds_ns.TransactionRow = __ds_scope.TransactionRow;
})();
