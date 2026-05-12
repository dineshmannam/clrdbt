use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DebtInput {
    pub name: String,
    pub balance: f64,
    pub interest_rate: f64, // annual percentage, e.g. 19.99
    pub min_payment: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct DebtResult {
    pub name: String,
    pub balance: f64,
    pub interest_rate: f64,
    pub min_payment: f64,
    pub payoff_date: String,  // "Month YYYY"
    pub payoff_month: u32,    // months from start of calculation
}

#[derive(Debug, Serialize)]
pub struct SnowballResult {
    pub debts: Vec<DebtResult>,
    pub debt_free_date: String,
    pub total_months: u32,
    pub total_balance: String,
    pub total_interest: String,
    pub monthly_payment: String,
    pub generated_date: String,
}

/// Run the debt snowball calculation.
///
/// Debts are sorted smallest balance first. Minimum payments are applied to all
/// debts each month. Any remaining budget is applied to the smallest remaining
/// debt. When a debt is paid off its minimum payment rolls into the extra budget.
pub fn calculate(mut debts: Vec<DebtInput>, monthly_payment: f64) -> SnowballResult {
    // Sort by balance ascending (snowball)
    debts.sort_by(|a, b| a.balance.partial_cmp(&b.balance).unwrap());

    let total_balance: f64 = debts.iter().map(|d| d.balance).sum();
    let n = debts.len();

    // Mutable state per debt
    let mut balances: Vec<f64> = debts.iter().map(|d| d.balance).collect();
    let mut payoff_months: Vec<Option<u32>> = vec![None; n];

    let today = OffsetDateTime::now_utc().date();
    let mut month = 0u32;
    let mut total_interest = 0.0f64;

    // Cap at 600 months (50 years) to avoid infinite loops
    while balances.iter().any(|&b| b > 0.01) && month < 600 {
        month += 1;

        // Accrue monthly interest on all remaining balances
        for i in 0..n {
            if balances[i] > 0.01 {
                let monthly_rate = debts[i].interest_rate / 100.0 / 12.0;
                let interest = balances[i] * monthly_rate;
                total_interest += interest;
                balances[i] += interest;
            }
        }

        // Apply minimum payments to all debts
        let mut remaining_budget = monthly_payment;
        for i in 0..n {
            if balances[i] > 0.01 {
                let payment = debts[i].min_payment.min(balances[i]);
                balances[i] -= payment;
                remaining_budget -= payment;
                if balances[i] <= 0.01 {
                    balances[i] = 0.0;
                    if payoff_months[i].is_none() {
                        payoff_months[i] = Some(month);
                    }
                }
            }
        }

        // Apply extra budget to smallest unpaid debt (first in sorted order)
        if remaining_budget > 0.01 {
            for i in 0..n {
                if balances[i] > 0.01 {
                    let payment = remaining_budget.min(balances[i]);
                    balances[i] -= payment;
                    if balances[i] <= 0.01 {
                        balances[i] = 0.0;
                        if payoff_months[i].is_none() {
                            payoff_months[i] = Some(month);
                        }
                    }
                    break; // only apply to one debt at a time
                }
            }
        }
    }

    let total_months = month;

    let debt_free_date = add_months(today, total_months);
    let debt_free_str = format_date(debt_free_date);

    let debts_result: Vec<DebtResult> = debts
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let payoff_m = payoff_months[i].unwrap_or(total_months);
            let payoff_date = format_date(add_months(today, payoff_m));
            DebtResult {
                name: d.name.clone(),
                balance: d.balance,
                interest_rate: d.interest_rate,
                min_payment: d.min_payment,
                payoff_date,
                payoff_month: payoff_m,
            }
        })
        .collect();

    SnowballResult {
        debts: debts_result,
        debt_free_date: debt_free_str,
        total_months,
        total_balance: format!("{:.2}", total_balance),
        total_interest: format!("{:.2}", total_interest),
        monthly_payment: format!("{:.2}", monthly_payment),
        generated_date: format_date(today),
    }
}

fn add_months(date: Date, months: u32) -> Date {
    // Simple month addition: advance by the given number of months
    let total_months = date.month() as u32 - 1 + months;
    let years_to_add = total_months / 12;
    let month_index = total_months % 12; // 0-based

    let new_year = date.year() + years_to_add as i32;
    let new_month = time::Month::try_from((month_index + 1) as u8).unwrap_or(time::Month::January);

    // Clamp day to the last day of the month
    let max_day = days_in_month(new_year, new_month);
    let new_day = date.day().min(max_day);

    Date::from_calendar_date(new_year, new_month, new_day).unwrap_or(date)
}

fn days_in_month(year: i32, month: time::Month) -> u8 {
    match month {
        time::Month::January | time::Month::March | time::Month::May
        | time::Month::July | time::Month::August | time::Month::October
        | time::Month::December => 31,
        time::Month::April | time::Month::June | time::Month::September
        | time::Month::November => 30,
        time::Month::February => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
    }
}

fn format_date(date: Date) -> String {
    let month_name = match date.month() {
        time::Month::January => "January",
        time::Month::February => "February",
        time::Month::March => "March",
        time::Month::April => "April",
        time::Month::May => "May",
        time::Month::June => "June",
        time::Month::July => "July",
        time::Month::August => "August",
        time::Month::September => "September",
        time::Month::October => "October",
        time::Month::November => "November",
        time::Month::December => "December",
    };
    format!("{} {}", month_name, date.year())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn debt(name: &str, balance: f64, rate: f64, min: f64) -> DebtInput {
        DebtInput { name: name.into(), balance, interest_rate: rate, min_payment: min }
    }

    // --- Snowball ordering ---

    #[test]
    fn debts_sorted_smallest_first() {
        let debts = vec![
            debt("Big",   5000.0, 10.0, 100.0),
            debt("Small",  500.0, 10.0,  50.0),
        ];
        let result = calculate(debts, 200.0);
        assert_eq!(result.debts[0].name, "Small");
        assert_eq!(result.debts[1].name, "Big");
    }

    // --- Zero-interest payoff math ---

    #[test]
    fn single_debt_zero_interest_exact_months() {
        // $1 200 at 0%, $100/month → exactly 12 months
        let result = calculate(vec![debt("Card", 1200.0, 0.0, 100.0)], 100.0);
        assert_eq!(result.total_months, 12);
    }

    #[test]
    fn single_debt_overpay_clears_faster() {
        // $600 at 0%, $200/month → 3 months, not 6
        let result = calculate(vec![debt("Card", 600.0, 0.0, 100.0)], 200.0);
        assert_eq!(result.total_months, 3);
    }

    // --- Snowball rollover ---

    #[test]
    fn minimum_rolls_over_after_payoff() {
        // Small: $500 at 0%, min $100. Extra budget = $300 - $100 - $100 = $100.
        // Small gets $200/month → paid in 3 months (500→300→100→0).
        // Big after 3 months of $100 min: $1700. Month 3 extra also hits Big: $1600.
        // Months 4–9: full $300 on Big → 1600/300 = 5.3 → 6 more months → total 9.
        let debts = vec![
            debt("Big",   2000.0, 0.0, 100.0),
            debt("Small",  500.0, 0.0, 100.0),
        ];
        let result = calculate(debts, 300.0);
        assert_eq!(result.total_months, 9);
    }

    #[test]
    fn small_debt_paid_before_big() {
        let debts = vec![
            debt("Big",   3000.0, 5.0, 60.0),
            debt("Small",  300.0, 5.0, 20.0),
        ];
        let result = calculate(debts, 150.0);
        // Small should have an earlier payoff month than Big
        let small = result.debts.iter().find(|d| d.name == "Small").unwrap();
        let big   = result.debts.iter().find(|d| d.name == "Big").unwrap();
        assert!(small.payoff_month <= big.payoff_month);
    }

    // --- Interest accrual ---

    #[test]
    fn interest_increases_total_paid() {
        let no_interest  = calculate(vec![debt("Card", 1000.0,  0.0, 50.0)], 100.0);
        let with_interest = calculate(vec![debt("Card", 1000.0, 20.0, 50.0)], 100.0);
        let no_int_paid: f64  = no_interest.total_interest.parse().unwrap();
        let with_int_paid: f64 = with_interest.total_interest.parse().unwrap();
        assert!(with_int_paid > no_int_paid);
        assert!(with_interest.total_months > no_interest.total_months);
    }

    // --- Output format ---

    #[test]
    fn debt_free_date_is_month_year_format() {
        let result = calculate(vec![debt("Card", 100.0, 0.0, 100.0)], 100.0);
        let parts: Vec<&str> = result.debt_free_date.split(' ').collect();
        assert_eq!(parts.len(), 2, "expected 'Month YYYY'");
        assert!(parts[1].parse::<i32>().is_ok(), "year should be a number");
    }

    #[test]
    fn totals_formatted_to_two_decimal_places() {
        let result = calculate(vec![debt("Card", 1000.0, 0.0, 100.0)], 100.0);
        assert!(result.total_balance.contains('.'));
        assert_eq!(result.total_balance.split('.').nth(1).unwrap().len(), 2);
    }

    // --- Edge cases ---

    #[test]
    fn single_month_payoff() {
        // Balance equals monthly payment → paid in 1 month
        let result = calculate(vec![debt("Card", 200.0, 0.0, 200.0)], 200.0);
        assert_eq!(result.total_months, 1);
    }

    #[test]
    fn minimum_payment_only_terminates() {
        // Even with no extra budget, calculation should still terminate
        let debts = vec![
            debt("A", 1000.0, 0.0,  50.0),
            debt("B", 2000.0, 0.0, 100.0),
        ];
        let result = calculate(debts, 150.0);
        assert!(result.total_months > 0);
        assert!(result.total_months < 600);
    }

    #[test]
    fn three_debts_snowball_order() {
        let debts = vec![
            debt("Medium", 2000.0, 5.0, 50.0),
            debt("Large",  5000.0, 8.0, 100.0),
            debt("Small",   500.0, 3.0, 20.0),
        ];
        let result = calculate(debts, 300.0);
        assert_eq!(result.debts[0].name, "Small");
        assert_eq!(result.debts[1].name, "Medium");
        assert_eq!(result.debts[2].name, "Large");
        // Each subsequent debt paid off no earlier than the previous
        assert!(result.debts[0].payoff_month <= result.debts[1].payoff_month);
        assert!(result.debts[1].payoff_month <= result.debts[2].payoff_month);
    }
}
