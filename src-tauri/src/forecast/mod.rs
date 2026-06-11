//! Forecast core — projected running balance ("saldo projetado", the methodology's heart).
//!
//! Pure functional core: NO IO, NO ambient clock, NO DB. Every input (seed, events, horizon,
//! "today") arrives as an argument, so the engine is deterministic and trivially testable. The
//! imperative shell (`commands.rs`) loads/maps rows and supplies the seed. See
//! `specs/003-forecast-core/`.
//!
//! The pure engine is complete (daily chain, month-end, deepest deficit, safe-to-spend, monthly
//! Totais). Remaining slice work is in the shell: wire the row→event mapping + seed into
//! `get_dashboard_summary` (Phase 7) and add a demo fixture (Phase 8).

// Public engine API. Some outputs (`deepest_deficit`, `safe_to_spend_today_cents`, `months`) are
// consumed by later slices (Mia decision tools, the Totais screen), so allow unread-for-now.
#![allow(dead_code)]

use chrono::{Datelike, NaiveDate};

/// A dated cash-flow event in the projection. Amounts are always positive; the sign is implied by
/// `kind`. `realized = false` marks a future projection (vs a realized transaction).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// Entrada — income (salary, reimbursement, freela…).
    Income,
    /// Saída — fixed outflow: fixed bills + the credit-invoice lump at the card due day (Régua 2).
    FixedOut,
    /// Diário — variable daily débito/cash spend (Régua 1).
    Daily,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CashflowEvent {
    pub date: NaiveDate,
    pub kind: EventKind,
    pub amount_cents: i64,
    pub realized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayPoint {
    pub date: NaiveDate,
    pub balance_cents: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonthEnd {
    pub year: i32,
    pub month: u32,
    pub balance_cents: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonthMetric {
    pub year: i32,
    pub month: u32,
    pub performance_cents: i64,
    pub cost_of_living_cents: i64,
    pub real_daily_avg_cents: i64,
    pub savings_rate_bps: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Forecast {
    /// Projected balance for each day from `today` to `horizon_end` inclusive.
    pub daily: Vec<DayPoint>,
    /// Projected balance on the last day of each month in the horizon.
    pub month_end: Vec<MonthEnd>,
    /// Lowest projected balance in the horizon and the day it occurs.
    pub deepest_deficit: Option<DayPoint>,
    /// Max extra outflow today keeping every future day's balance >= 0.
    pub safe_to_spend_today_cents: i64,
    /// Per-month decision metrics (Totais).
    pub months: Vec<MonthMetric>,
}

/// Net signed effect of an event on the balance (income adds, outflows subtract).
fn signed(e: &CashflowEvent) -> i64 {
    match e.kind {
        EventKind::Income => e.amount_cents,
        EventKind::FixedOut | EventKind::Daily => -e.amount_cents,
    }
}

/// Row→event classification rule (the shell maps DB rows through this).
/// `income` → Entrada; an `expense` on credit or marked fixed → Saída (a fatura lump or fixed bill);
/// any other `expense` → Diário (variable débito/cash); `transfer` is skipped (net-zero between
/// own accounts in this slice — economia/transfer-to-savings is a later slice).
pub fn classify(txn_type: &str, is_fixed: bool, payment_method: Option<&str>) -> Option<EventKind> {
    match txn_type {
        "income" => Some(EventKind::Income),
        "expense" => {
            if is_fixed || payment_method == Some("credit") {
                Some(EventKind::FixedOut)
            } else {
                Some(EventKind::Daily)
            }
        }
        _ => None,
    }
}

/// Last calendar day of the given month.
pub fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(ny, nm, 1)
        .and_then(|first_of_next| first_of_next.pred_opt())
        .expect("valid month")
}

/// Given a checkin date and a card's closing/due days, compute the due date for that cycle.
/// The cycle closes on `closing_day` of the checkin's month (or previous month if checkin is before
/// closing_day), and the invoice is due on `due_day` of the following month.
pub fn cycle_due_date(checkin_date: NaiveDate, closing_day: u32, due_day: u32) -> NaiveDate {
    let (cycle_close_year, cycle_close_month) = if checkin_date.day() <= closing_day {
        // Checkin is before or on closing_day → cycle closes this month
        (checkin_date.year(), checkin_date.month())
    } else {
        // Checkin is after closing_day → cycle closed last month
        if checkin_date.month() == 1 {
            (checkin_date.year() - 1, 12)
        } else {
            (checkin_date.year(), checkin_date.month() - 1)
        }
    };

    // Due date is in the month AFTER the cycle closes
    let (due_year, due_month) = if cycle_close_month == 12 {
        (cycle_close_year + 1, 1)
    } else {
        (cycle_close_year, cycle_close_month + 1)
    };

    let last_day = last_day_of_month(due_year, due_month);
    let due_day_clamped = due_day.min(last_day.day());
    NaiveDate::from_ymd_opt(due_year, due_month, due_day_clamped).expect("valid due date")
}

/// Projected balance on each month's last day within the (chronological) daily series.
fn month_end_points(daily: &[DayPoint]) -> Vec<MonthEnd> {
    let mut out: Vec<MonthEnd> = Vec::new();
    for p in daily {
        let (year, month) = (p.date.year(), p.date.month());
        match out.last_mut() {
            Some(last) if last.year == year && last.month == month => {
                last.balance_cents = p.balance_cents;
            }
            _ => out.push(MonthEnd {
                year,
                month,
                balance_cents: p.balance_cents,
            }),
        }
    }
    out
}

/// Lowest projected balance and its earliest date (None if the series is empty).
fn deepest(daily: &[DayPoint]) -> Option<DayPoint> {
    daily.iter().copied().reduce(|a, b| {
        if b.balance_cents < a.balance_cents {
            b
        } else {
            a
        }
    })
}

/// Per-month decision metrics (Totais). Metrics cover the **whole month** (realized so far +
/// projected), so they filter `events` by month, not by horizon. `today` bounds "elapsed days"
/// for the real daily average (kept as an argument — no ambient clock).
fn month_metrics(
    today: NaiveDate,
    events: &[CashflowEvent],
    months: &[MonthEnd],
) -> Vec<MonthMetric> {
    months
        .iter()
        .map(|me| {
            let (year, month) = (me.year, me.month);
            let mut income = 0i64;
            let mut fixed_out = 0i64;
            let mut daily = 0i64;
            let mut realized_daily = 0i64;
            for e in events
                .iter()
                .filter(|e| e.date.year() == year && e.date.month() == month)
            {
                match e.kind {
                    EventKind::Income => income += e.amount_cents,
                    EventKind::FixedOut => fixed_out += e.amount_cents,
                    EventKind::Daily => {
                        daily += e.amount_cents;
                        if e.realized {
                            realized_daily += e.amount_cents;
                        }
                    }
                }
            }
            let cost_of_living_cents = fixed_out + daily; // credit lumps are FixedOut
            let performance_cents = income - cost_of_living_cents;

            let first = NaiveDate::from_ymd_opt(year, month, 1).expect("valid month");
            let last = last_day_of_month(year, month);
            let elapsed = if today < first {
                0
            } else {
                let end = if today > last { last } else { today };
                (end - first).num_days() + 1
            };
            let real_daily_avg_cents = if elapsed > 0 {
                realized_daily / elapsed
            } else {
                0
            };

            // Provisional: "saved" = surplus (performance). An explicit economia / transfer-to-
            // savings event is a later slice; for now the rate flags the 20–30% band off surplus.
            let savings_rate_bps = if income > 0 {
                performance_cents.max(0) * 10_000 / income
            } else {
                0
            };

            MonthMetric {
                year,
                month,
                performance_cents,
                cost_of_living_cents,
                real_daily_avg_cents,
                savings_rate_bps,
            }
        })
        .collect()
}

/// Project the running cash balance day by day from `today` to `horizon_end` (inclusive).
///
/// `seed_cents` is the opening balance carried into `today` (before today's events); thus
/// `daily[0].balance = seed + net(events on today)`, mirroring the spreadsheet's
/// `Saldo[d] = Saldo[d-1] + Entrada − (Saída + Diário)`.
pub fn project(
    seed_cents: i64,
    today: NaiveDate,
    events: &[CashflowEvent],
    horizon_end: NaiveDate,
) -> Forecast {
    let mut daily = Vec::new();
    let mut balance = seed_cents;
    let mut day = today;
    while day <= horizon_end {
        let net: i64 = events.iter().filter(|e| e.date == day).map(signed).sum();
        balance += net;
        daily.push(DayPoint {
            date: day,
            balance_cents: balance,
        });
        day = match day.succ_opt() {
            Some(next) => next,
            None => break, // chrono's max representable date; horizons never reach this
        };
    }
    let month_end = month_end_points(&daily);
    let deepest_deficit = deepest(&daily);
    let safe_to_spend_today_cents = deepest_deficit.map(|p| p.balance_cents.max(0)).unwrap_or(0);
    let months = month_metrics(today, events, &month_end);

    Forecast {
        daily,
        month_end,
        deepest_deficit,
        safe_to_spend_today_cents,
        months,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }
    fn ev(date: &str, kind: EventKind, amount_cents: i64) -> CashflowEvent {
        CashflowEvent {
            date: d(date),
            kind,
            amount_cents,
            realized: true,
        }
    }

    // T2.4 — empty events yield a flat line at the seed for every day in the horizon.
    #[test]
    fn empty_events_flat_seed_line() {
        let f = project(100000, d("2026-01-01"), &[], d("2026-01-03"));
        assert_eq!(f.daily.len(), 3);
        assert!(f.daily.iter().all(|p| p.balance_cents == 100000));
        assert_eq!(f.daily[0].date, d("2026-01-01"));
        assert_eq!(f.daily[2].date, d("2026-01-03"));
    }

    // T2.1 — single-month chain: saldo[d] = saldo[d-1] + income − (fixed_out + daily).
    #[test]
    fn single_month_chain() {
        let events = [
            ev("2026-01-01", EventKind::Income, 200000),
            ev("2026-01-02", EventKind::FixedOut, 50000),
            ev("2026-01-02", EventKind::Daily, 30000),
            ev("2026-01-03", EventKind::Daily, 20000),
        ];
        let f = project(1000000, d("2026-01-01"), &events, d("2026-01-03"));
        assert_eq!(f.daily[0].balance_cents, 1200000); // 1000 + 200
        assert_eq!(f.daily[1].balance_cents, 1120000); // 1200 - 50 - 30
        assert_eq!(f.daily[2].balance_cents, 1100000); // 1120 - 20
    }

    // T2.2 — month boundary: last day of a month seeds the first day of the next.
    #[test]
    fn month_boundary_carries() {
        let events = [
            ev("2026-01-31", EventKind::Daily, 100000),
            ev("2026-02-01", EventKind::Income, 500000),
        ];
        let f = project(300000, d("2026-01-31"), &events, d("2026-02-01"));
        assert_eq!(f.daily[0].date, d("2026-01-31"));
        assert_eq!(f.daily[0].balance_cents, 200000); // 300 - 100
        assert_eq!(f.daily[1].date, d("2026-02-01"));
        assert_eq!(f.daily[1].balance_cents, 700000); // 200 + 500
    }

    // T2.3 — year boundary (Dec → Jan) continuity.
    #[test]
    fn year_boundary_carries() {
        let events = [
            ev("2025-12-31", EventKind::FixedOut, 80000),
            ev("2026-01-01", EventKind::Income, 600000),
        ];
        let f = project(1000000, d("2025-12-31"), &events, d("2026-01-01"));
        assert_eq!(f.daily.len(), 2);
        assert_eq!(f.daily[0].balance_cents, 920000); // Dec 31 2025: 1000 - 80
        assert_eq!(f.daily[1].date, d("2026-01-01"));
        assert_eq!(f.daily[1].balance_cents, 1520000); // Jan 1 2026: 920 + 600
    }

    // T2.4 — determinism: identical inputs yield identical output.
    #[test]
    fn deterministic() {
        let events = [ev("2026-01-02", EventKind::Daily, 42000)];
        let a = project(500000, d("2026-01-01"), &events, d("2026-01-05"));
        let b = project(500000, d("2026-01-01"), &events, d("2026-01-05"));
        assert_eq!(a.daily, b.daily);
        assert_eq!(a.daily.len(), 5);
    }

    // ---- Phase 3: month-end (US3) + deepest deficit (US4) ----

    // T3.1 — month_end is the projected balance on each month's last day within the horizon.
    #[test]
    fn month_end_per_month() {
        let events = [
            ev("2026-01-31", EventKind::Income, 200000),
            ev("2026-02-02", EventKind::FixedOut, 50000),
        ];
        let f = project(1000000, d("2026-01-30"), &events, d("2026-02-02"));
        assert_eq!(f.month_end.len(), 2);
        assert_eq!((f.month_end[0].year, f.month_end[0].month), (2026, 1));
        assert_eq!(f.month_end[0].balance_cents, 1200000); // Jan 31: 1000 + 200
        assert_eq!((f.month_end[1].year, f.month_end[1].month), (2026, 2));
        assert_eq!(f.month_end[1].balance_cents, 1150000); // Feb 2: 1200 - 50
    }

    // T3.2 — deepest deficit = min projected balance + its (earliest) date, negative trough.
    #[test]
    fn deepest_deficit_negative() {
        let events = [ev("2026-01-02", EventKind::FixedOut, 1500000)];
        let f = project(1000000, d("2026-01-01"), &events, d("2026-01-04"));
        let dd = f.deepest_deficit.unwrap();
        assert_eq!(dd.balance_cents, -500000);
        assert_eq!(dd.date, d("2026-01-02"));
    }

    // T3.3 — all-positive horizon → deepest deficit is the minimum positive trough.
    #[test]
    fn deepest_deficit_positive_trough() {
        let events = [
            ev("2026-01-02", EventKind::Daily, 300000),
            ev("2026-01-03", EventKind::Income, 500000),
        ];
        let f = project(1000000, d("2026-01-01"), &events, d("2026-01-03"));
        let dd = f.deepest_deficit.unwrap();
        assert_eq!(dd.balance_cents, 700000);
        assert_eq!(dd.date, d("2026-01-02"));
    }

    // ---- Phase 4: safe-to-spend today (US5) ----

    // T4.1 / T4.3 — safe-to-spend equals the min future balance (spending it makes the trough touch 0).
    #[test]
    fn safe_to_spend_equals_min_balance() {
        let events = [ev("2026-01-03", EventKind::FixedOut, 200000)];
        let f = project(500000, d("2026-01-01"), &events, d("2026-01-04"));
        assert_eq!(f.safe_to_spend_today_cents, 300000); // min over horizon
    }

    // T4.2 — already negative ahead → safe-to-spend clamps to 0, never negative.
    #[test]
    fn safe_to_spend_zero_when_negative() {
        let events = [ev("2026-01-02", EventKind::FixedOut, 800000)];
        let f = project(500000, d("2026-01-01"), &events, d("2026-01-03"));
        assert_eq!(f.safe_to_spend_today_cents, 0);
    }

    // ---- Phase 5: monthly metrics / Totais (US6) ----

    // T5.1 — performance = income − all_out; cost_of_living = fixed_out + daily (+ card via FixedOut).
    #[test]
    fn month_performance_and_cost() {
        let events = [
            ev("2026-03-05", EventKind::Income, 1000000),
            ev("2026-03-10", EventKind::FixedOut, 400000),
            ev("2026-03-12", EventKind::Daily, 200000),
        ];
        let f = project(0, d("2026-03-01"), &events, d("2026-03-31"));
        let m = f.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(m.cost_of_living_cents, 600000); // 400 + 200
        assert_eq!(m.performance_cents, 400000); // 1000 - 600
    }

    // T5.3 — cash ≠ performance: month ends negative in cash while performance is positive.
    #[test]
    fn cash_differs_from_performance() {
        let events = [
            ev("2026-03-01", EventKind::Income, 100000),
            ev("2026-03-02", EventKind::FixedOut, 20000),
        ];
        let f = project(-200000, d("2026-03-01"), &events, d("2026-03-02"));
        let m = f.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(m.performance_cents, 80000); // 100 - 20 (positive)
        assert_eq!(f.month_end[0].balance_cents, -120000); // cash ends negative
    }

    // T5.2 — real daily average = realized daily ÷ elapsed days; savings rate in basis points.
    #[test]
    fn real_daily_avg_and_savings() {
        let mut events = vec![ev("2026-03-01", EventKind::Income, 1000000)];
        for day in ["2026-03-02", "2026-03-04", "2026-03-06", "2026-03-08"] {
            events.push(ev(day, EventKind::Daily, 50000)); // realized daily, 4 × 50 = 200
        }
        let f = project(0, d("2026-03-10"), &events, d("2026-03-31"));
        let m = f.months.iter().find(|m| m.month == 3).unwrap();
        assert_eq!(m.real_daily_avg_cents, 20000); // 200.00 / 10 elapsed days
        assert_eq!(m.savings_rate_bps, 8000); // (1000 - 200) / 1000 = 80% = 8000 bps
    }

    // ---- Phase 6: credit dual-tracking (US7) ----

    // T6.1 — a Daily event (Régua 1, débito) reduces the balance on its own day.
    #[test]
    fn regua1_daily_hits_its_day() {
        let events = [ev("2026-01-02", EventKind::Daily, 70000)];
        let f = project(1000000, d("2026-01-01"), &events, d("2026-01-02"));
        assert_eq!(f.daily[0].balance_cents, 1000000); // Jan 1 untouched
        assert_eq!(f.daily[1].balance_cents, 930000); // Jan 2: −70
    }

    // T6.2 / T6.3 — a credit lump (Régua 2) lands as one FixedOut on the due day, depressing the
    // future month, while débito daily (Régua 1) only touches its own day.
    #[test]
    fn regua2_credit_lump_at_due_day() {
        let events = [
            ev("2026-01-10", EventKind::Daily, 20000), // débito daily (Régua 1)
            ev("2026-02-15", EventKind::FixedOut, 600000), // invoice lump at due day (Régua 2)
        ];
        let f = project(1000000, d("2026-01-10"), &events, d("2026-02-15"));
        let jan = f.month_end.iter().find(|m| m.month == 1).unwrap();
        let feb = f.month_end.iter().find(|m| m.month == 2).unwrap();
        assert_eq!(jan.balance_cents, 980000); // 1000 − 20 (only daily)
        assert_eq!(feb.balance_cents, 380000); // 980 − 600 at Feb 15
    }

    // ---- Phase 7: row→event classification (US8 mapping) ----

    // T7.1 — classify maps raw transaction rows to the right event kind.
    #[test]
    fn classify_maps_rows_to_kinds() {
        assert_eq!(classify("income", false, None), Some(EventKind::Income));
        assert_eq!(
            classify("expense", true, Some("debit")),
            Some(EventKind::FixedOut)
        ); // fixed bill
        assert_eq!(
            classify("expense", false, Some("credit")),
            Some(EventKind::FixedOut)
        ); // credit lump
        assert_eq!(
            classify("expense", false, Some("debit")),
            Some(EventKind::Daily)
        ); // variable débito
        assert_eq!(classify("expense", false, None), Some(EventKind::Daily));
        assert_eq!(classify("transfer", false, None), None); // skipped (net-zero)
    }

    // ---- Phase 7: credit cycle aggregation (T7.2) ----

    // T7.2a — cycle_due_date: checkin before closing_day → due next month
    #[test]
    fn cycle_due_date_before_closing() {
        // Card closes on day 20, due on day 10
        // Checkin on Jan 15 (before closing) → cycle closes Jan 20 → due Feb 10
        let checkin = d("2026-01-15");
        let due = cycle_due_date(checkin, 20, 10);
        assert_eq!(due, d("2026-02-10"));
    }

    // T7.2b — cycle_due_date: checkin after closing_day → due in 2 months
    #[test]
    fn cycle_due_date_after_closing() {
        // Card closes on day 20, due on day 10
        // Checkin on Jan 25 (after closing) → cycle closed Dec 20 → due Jan 10
        let checkin = d("2026-01-25");
        let due = cycle_due_date(checkin, 20, 10);
        assert_eq!(due, d("2026-01-10"));
    }

    // T7.2c — cycle_due_date: year boundary
    #[test]
    fn cycle_due_date_year_boundary() {
        // Card closes on day 20, due on day 10
        // Checkin on Dec 15 → cycle closes Dec 20 → due Jan 10 next year
        let checkin = d("2025-12-15");
        let due = cycle_due_date(checkin, 20, 10);
        assert_eq!(due, d("2026-01-10"));
    }

    // T7.2d — cycle_due_date: due_day clamped to last day of month
    #[test]
    fn cycle_due_date_clamped() {
        // Card closes on day 20, due on day 31
        // Checkin on Jan 15 → cycle closes Jan 20 → due Feb 28 (clamped)
        let checkin = d("2026-01-15");
        let due = cycle_due_date(checkin, 20, 31);
        assert_eq!(due, d("2026-02-28"));
    }
}
