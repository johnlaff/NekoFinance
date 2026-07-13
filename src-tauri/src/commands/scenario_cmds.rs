use super::*;
use crate::scenarios::{self, ScenarioLoanInput, ScenarioLoanRow};

#[tauri::command]
pub async fn create_scenario_loan_cmd(
    pool: State<'_, SqlitePool>,
    input: ScenarioLoanInput,
) -> Result<String, String> {
    scenarios::create_scenario_loan(pool.inner(), input).await
}

#[tauri::command]
pub async fn update_scenario_loan_cmd(
    pool: State<'_, SqlitePool>,
    loan_id: String,
    input: ScenarioLoanInput,
) -> Result<(), String> {
    scenarios::update_scenario_loan(pool.inner(), &loan_id, input).await
}

#[tauri::command]
pub async fn delete_scenario_loan_cmd(
    pool: State<'_, SqlitePool>,
    scenario_id: String,
    loan_id: String,
) -> Result<(), String> {
    scenarios::delete_scenario_loan(pool.inner(), &scenario_id, &loan_id).await
}

#[tauri::command]
pub async fn list_scenario_loans_cmd(
    pool: State<'_, SqlitePool>,
    scenario_id: String,
) -> Result<Vec<ScenarioLoanRow>, String> {
    scenarios::list_scenario_loans(pool.inner(), &scenario_id).await
}
