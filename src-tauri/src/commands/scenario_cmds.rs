use super::*;
use crate::scenarios::{self, ScenarioLoanInput};

#[tauri::command]
pub async fn create_scenario_loan_cmd(
    pool: State<'_, SqlitePool>,
    input: ScenarioLoanInput,
) -> Result<(), String> {
    scenarios::create_scenario_loan(pool.inner(), input).await
}
