use chrono::{Datelike, NaiveDate};

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

/// O dia pedido, encurtado para o último dia do mês quando ele não cabe (29–31 em fevereiro ou
/// em qualquer mês de 30 dias). Um dia que não cabe é problema de derivar a data, não do
/// cadastro — encurtar preserva a intenção; recuar para um dia fixo menor mudaria a intenção.
pub fn clamp_day_of_month(day: u32, year: i32, month: u32) -> u32 {
    day.clamp(1, last_day_of_month(year, month).day())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_day_of_month_handles_december_rollover() {
        assert_eq!(
            last_day_of_month(2026, 12),
            NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()
        );
    }

    #[test]
    fn last_day_of_month_handles_leap_february() {
        assert_eq!(
            last_day_of_month(2024, 2),
            NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()
        );
    }

    #[test]
    fn last_day_of_month_handles_non_leap_february() {
        assert_eq!(
            last_day_of_month(2026, 2),
            NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
        );
    }

    #[test]
    fn clamp_day_of_month_shortens_to_last_day_when_out_of_range() {
        assert_eq!(clamp_day_of_month(31, 2026, 2), 28);
        assert_eq!(clamp_day_of_month(30, 2026, 4), 30);
        assert_eq!(clamp_day_of_month(31, 2024, 2), 29);
    }

    #[test]
    fn clamp_day_of_month_keeps_day_that_already_fits() {
        assert_eq!(clamp_day_of_month(15, 2026, 6), 15);
    }

    #[test]
    fn clamp_day_of_month_floors_zero_to_first_day() {
        assert_eq!(clamp_day_of_month(0, 2026, 6), 1);
    }
}
