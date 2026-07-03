import "./mia.css";
import { useState } from "react";
import { Send, Sparkles } from "lucide-react";
import { Button } from "../design-system/components/Button";
import { MiaAvatar } from "../design-system/components/MiaAvatar";
import { getDashboardSummary, getForecast, isTauri } from "../lib/api";
import { useCommand } from "../lib/useCommand";
import { fmtBRL } from "../lib/nkFormat";

/* ------------------------------------------------------------------ */
/* Suggestion chips (static, seeded)                                   */
/* ------------------------------------------------------------------ */

const SUGG_CHIPS = [
  "Por que o saldo cai em agosto?",
  "Resumo do mês",
  "Onde gastei mais?",
  "Pré-lançar o próximo mês",
];

/* ------------------------------------------------------------------ */
/* Message shape                                                        */
/* ------------------------------------------------------------------ */

type Sender = "mia" | "user";

interface Message {
  id: number;
  sender: Sender;
  text: string;
  calc?: string;
}

let _nextId = 1;
function nextId() {
  return _nextId++;
}

/* ------------------------------------------------------------------ */
/* Deterministic "can-spend today" answer                              */
/* ------------------------------------------------------------------ */

interface SpendAnswer {
  safe: number;
  ceiling: number;
  spent: number;
  remaining: number;
}

function buildSpendAnswer(
  dailyBudget: number,
  dailySpendToday: number,
  safeToSpendTodayCents: number,
): SpendAnswer {
  const ceiling = dailyBudget;
  const spent = dailySpendToday;
  const safe = Math.max(0, safeToSpendTodayCents);
  const remaining = ceiling - spent;
  return { safe, ceiling, spent, remaining };
}

// O valor promovido é o guardrail do motor (caixa + meta de poupança) — o teto diário é
// a outra régua, mostrada na trilha de cálculo para o usuário comparar as duas.
function spendAnswerText(ans: SpendAnswer): string {
  return (
    `Hoje você pode gastar até ` +
    `**${fmtBRL(ans.safe)}** sem deixar nenhum dia no vermelho nem comprometer a poupança do ano.`
  );
}

function spendAnswerCalc(ans: SpendAnswer): string {
  return (
    `limite pelo caixa e poupança = ${fmtBRL(ans.safe)}\n` +
    `teto diário = ${fmtBRL(ans.ceiling)}\n` +
    `já gasto hoje = ${fmtBRL(ans.spent)}\n` +
    `livre pelo teto = ${fmtBRL(ans.remaining)}`
  );
}

/* ------------------------------------------------------------------ */
/* Greeting + seeded conversation                                       */
/* ------------------------------------------------------------------ */

function buildInitialMessages(ans: SpendAnswer | null): Message[] {
  const greeting: Message = {
    id: nextId(),
    sender: "mia",
    text: "Oi! Posso explicar seus números, achar cobranças e sugerir lançamentos. Toda alteração na planilha passa pela sua aprovação.",
  };

  if (!ans) {
    return [greeting];
  }

  const userQ: Message = {
    id: nextId(),
    sender: "user",
    text: "Quanto posso gastar hoje?",
  };

  const miaReply: Message = {
    id: nextId(),
    sender: "mia",
    text: spendAnswerText(ans),
    calc: spendAnswerCalc(ans),
  };

  return [greeting, userQ, miaReply];
}

/* ------------------------------------------------------------------ */
/* Render a single bubble text (bold via **…**)                        */
/* ------------------------------------------------------------------ */

function BubbleText({ text }: { text: string }) {
  const parts = text.split(/\*\*(.+?)\*\*/g);
  return (
    <>
      {parts.map((part, i) =>
        i % 2 === 1 ? <strong key={`b-${i}-${part}`}>{part}</strong> : part,
      )}
    </>
  );
}

/* ------------------------------------------------------------------ */
/* Avatar shorthand                                                     */
/* ------------------------------------------------------------------ */

function MiaAv() {
  return (
    <span className="mia-av">
      <MiaAvatar width={32} height={32} />
    </span>
  );
}

/* ------------------------------------------------------------------ */
/* CopilotScreen                                                        */
/* ------------------------------------------------------------------ */

export function CopilotScreen() {
  const summaryQ = useCommand("get_dashboard_summary", getDashboardSummary);
  const forecastQ = useCommand("get_forecast", getForecast);

  const summary = summaryQ.data;
  const forecast = forecastQ.data;

  const loading = summaryQ.loading || forecastQ.loading;

  // Build the spend answer once we have data (undefined when not in Tauri / still loading)
  const spendAns: SpendAnswer | null =
    summary && forecast
      ? buildSpendAnswer(
          summary.daily_budget,
          summary.daily_spend_today,
          forecast.safe_to_spend_today_cents,
        )
      : null;

  const [messages, setMessages] = useState<Message[]>(() => buildInitialMessages(null));
  const [seeded, setSeeded] = useState(false);
  const [input, setInput] = useState("");

  // Once data arrives, inject the seeded conversation (once only)
  if (spendAns && !seeded) {
    setSeeded(true);
    setMessages(buildInitialMessages(spendAns));
  }

  function submitMessage(text: string) {
    const trimmed = text.trim();
    if (!trimmed) return;
    const userMsg: Message = {
      id: nextId(),
      sender: "user",
      text: trimmed,
    };
    const miaReply: Message = {
      id: nextId(),
      sender: "mia",
      text: "Ainda estou aprendendo a responder essa pergunta. Por enquanto, consulte os números no painel principal.",
    };
    setMessages((prev) => [...prev, userMsg, miaReply]);
    setInput("");
  }

  function handleChip(chip: string) {
    submitMessage(chip);
  }

  function handleSend() {
    submitMessage(input);
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter") handleSend();
  }

  return (
    <div className="xs">
      {/* Title bar */}
      <div
        className="xs-title"
        style={{ display: "flex", alignItems: "center", gap: 10 }}
      >
        <span className="mia-av" style={{ width: 30, height: 30 }}>
          <MiaAvatar width={30} height={30} />
        </span>
        Mia
        <span className="mia-badge">Lê sua planilha · responde local</span>
      </div>

      {/* Chat area */}
      <div className="mia">
        {/* Message stream */}
        <div className="mia-stream">
          {loading && !seeded ? (
            /* Quiet skeleton while first fetch is in-flight */
            <>
              <div className="mia-msg">
                <MiaAv />
                <div className="mia-bub" style={{ width: "60%" }}>
                  <div
                    className="mia-skeleton"
                    style={{ width: "80%", marginBottom: 6 }}
                  />
                  <div className="mia-skeleton" style={{ width: "55%" }} />
                </div>
              </div>
            </>
          ) : (
            messages.map((msg) => (
              <div
                key={msg.id}
                className={`mia-msg${msg.sender === "user" ? " mia-msg--user" : ""}`}
              >
                {msg.sender === "mia" && <MiaAv />}
                <div className="mia-bub">
                  <BubbleText text={msg.text} />
                  {msg.calc && <div className="mia-calc">{msg.calc}</div>}
                </div>
              </div>
            ))
          )}

          {/* Web-preview notice */}
          {!isTauri && !loading && (
            <div
              style={{
                color: "var(--text-faint)",
                fontSize: 12,
                padding: "8px 0",
              }}
            >
              Preview web — abra o app desktop para ver seus dados reais.
            </div>
          )}
        </div>

        {/* Bottom: chips + input */}
        <div>
          <div className="mia-sugg">
            {SUGG_CHIPS.map((s) => (
              <button
                type="button"
                key={s}
                className="mia-chip"
                onClick={() => handleChip(s)}
              >
                {s}
              </button>
            ))}
          </div>
          <div className="mia-input">
            <Sparkles
              size={16}
              strokeWidth={1.75}
              style={{ color: "var(--primary)", flexShrink: 0 }}
            />
            <input
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Pergunte à Mia sobre suas finanças…"
              aria-label="Mensagem para a Mia"
            />
            <Button
              variant="primary"
              size="sm"
              iconLeft={<Send size={14} strokeWidth={1.75} />}
              onClick={handleSend}
              disabled={!input.trim()}
            >
              Enviar
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
