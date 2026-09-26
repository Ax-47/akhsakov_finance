//! Server functions for monthly reports; delegate to
//! [`ReportService`](super::ReportService), injected via `Extension`.

use dioxus::prelude::*;
use dtos::report::MonthlyReport;
use uuid::Uuid;

#[cfg(feature = "server")]
use super::ReportService;
#[cfg(feature = "server")]
use dioxus::server::axum::Extension;

/// The report for `month` (`YYYY-MM`) of one portfolio, or all holdings.
/// `today` (`YYYY-MM-DD`, the viewer's date) picks how much history to load.
#[post("/api/reports/monthly", service: Extension<ReportService>)]
pub async fn get_monthly_report(
    month: String,
    portfolio: Option<Uuid>,
    today: String,
) -> Result<MonthlyReport, ServerFnError> {
    Ok(service.build(&month, portfolio, &today).await?)
}

/// Sends the report for `month` to the app and your push channels now.
#[post("/api/reports/monthly/send", service: Extension<ReportService>)]
pub async fn send_monthly_report(month: String, today: String) -> Result<(), ServerFnError> {
    Ok(service.send(&month, &today).await?)
}

/// Whether last month's report is sent automatically on the 1st.
#[get("/api/reports/schedule", service: Extension<ReportService>)]
pub async fn get_report_schedule() -> Result<bool, ServerFnError> {
    Ok(service.enabled()?)
}

#[post("/api/reports/schedule/save", service: Extension<ReportService>)]
pub async fn set_report_schedule(on: bool) -> Result<(), ServerFnError> {
    Ok(service.set_enabled(on)?)
}
