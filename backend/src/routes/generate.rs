use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use uuid::Uuid;

use crate::{calculation, pdf, storage};

#[derive(Deserialize)]
pub struct GenerateRequest {
    pub monthly_payment: f64,
    pub debts: Vec<calculation::DebtInput>,
}

#[derive(Serialize)]
pub struct GenerateResponse {
    pub download_url: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn validate(req: &GenerateRequest) -> Result<(), String> {
    if req.debts.is_empty() {
        return Err("At least one debt is required.".into());
    }

    if !req.monthly_payment.is_finite() || req.monthly_payment <= 0.0 {
        return Err("Monthly payment must be a positive number.".into());
    }

    for debt in &req.debts {
        if !debt.balance.is_finite() || debt.balance <= 0.0 {
            return Err(format!(
                "'{}': balance must be a positive number.",
                debt.name
            ));
        }
        if !debt.interest_rate.is_finite() || debt.interest_rate < 0.0 {
            return Err(format!(
                "'{}': interest rate must be 0 or greater.",
                debt.name
            ));
        }
        if debt.interest_rate > 1000.0 {
            return Err(format!(
                "'{}': interest rate seems unreasonably high (> 1000%).",
                debt.name
            ));
        }
        if !debt.min_payment.is_finite() || debt.min_payment < 0.0 {
            return Err(format!(
                "'{}': minimum payment must be 0 or greater.",
                debt.name
            ));
        }
    }

    let total_min: f64 = req.debts.iter().map(|d| d.min_payment).sum();
    if req.monthly_payment < total_min {
        return Err(format!(
            "Monthly payment must be at least ${:.2} (sum of minimums).",
            total_min
        ));
    }

    Ok(())
}

pub async fn handle(
    Json(req): Json<GenerateRequest>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    if let Err(msg) = validate(&req) {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: msg })));
    }

    let result = calculation::calculate(req.debts, req.monthly_payment);
    info!("Debt-free date calculated: {}", result.debt_free_date);

    let pdf_bytes = match pdf::render_pdf(&result).await {
        Ok(b) => b,
        Err(e) => {
            error!("PDF generation failed: {e}");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "PDF generation failed.".into(),
                }),
            ));
        }
    };

    let pdf_id = Uuid::new_v4().to_string();

    if let Err(e) = storage::upload_pdf(&pdf_id, pdf_bytes).await {
        error!("GCS upload failed: {e}");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Storage failed.".into(),
            }),
        ));
    }

    let download_url = match storage::signed_url(&pdf_id).await {
        Ok(u) => u,
        Err(e) => {
            error!("Signed URL generation failed: {e}");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Download URL generation failed.".into(),
                }),
            ));
        }
    };

    info!("PDF ready: {pdf_id}");
    Ok((StatusCode::OK, Json(GenerateResponse { download_url })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::post,
        Router,
    };
    use tower::ServiceExt;

    fn app() -> Router {
        Router::new().route("/api/generate", post(handle))
    }

    async fn post_json(app: Router, body: serde_json::Value) -> axum::response::Response {
        let req = Request::builder()
            .method("POST")
            .uri("/api/generate")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        app.oneshot(req).await.unwrap()
    }

    fn req(monthly_payment: f64, debts: Vec<calculation::DebtInput>) -> GenerateRequest {
        GenerateRequest {
            monthly_payment,
            debts,
        }
    }

    fn debt(name: &str, balance: f64, rate: f64, min: f64) -> calculation::DebtInput {
        calculation::DebtInput {
            name: name.into(),
            balance,
            interest_rate: rate,
            min_payment: min,
        }
    }

    // --- validate() unit tests (no HTTP, no PDF, no GCS) ---

    #[test]
    fn rejects_empty_debts() {
        assert!(validate(&req(500.0, vec![])).is_err());
    }

    #[test]
    fn rejects_zero_monthly_payment() {
        assert!(validate(&req(0.0, vec![debt("A", 1000.0, 10.0, 50.0)])).is_err());
    }

    #[test]
    fn rejects_negative_monthly_payment() {
        assert!(validate(&req(-100.0, vec![debt("A", 1000.0, 10.0, 50.0)])).is_err());
    }

    #[test]
    fn rejects_nan_monthly_payment() {
        assert!(validate(&req(f64::NAN, vec![debt("A", 1000.0, 10.0, 50.0)])).is_err());
    }

    #[test]
    fn rejects_infinite_monthly_payment() {
        assert!(validate(&req(f64::INFINITY, vec![debt("A", 1000.0, 10.0, 50.0)])).is_err());
    }

    #[test]
    fn rejects_negative_balance() {
        assert!(validate(&req(200.0, vec![debt("A", -500.0, 10.0, 50.0)])).is_err());
    }

    #[test]
    fn rejects_zero_balance() {
        assert!(validate(&req(200.0, vec![debt("A", 0.0, 10.0, 50.0)])).is_err());
    }

    #[test]
    fn rejects_nan_balance() {
        assert!(validate(&req(200.0, vec![debt("A", f64::NAN, 10.0, 50.0)])).is_err());
    }

    #[test]
    fn rejects_negative_interest_rate() {
        assert!(validate(&req(200.0, vec![debt("A", 1000.0, -5.0, 50.0)])).is_err());
    }

    #[test]
    fn rejects_absurd_interest_rate() {
        assert!(validate(&req(200.0, vec![debt("A", 1000.0, 1001.0, 50.0)])).is_err());
    }

    #[test]
    fn rejects_nan_interest_rate() {
        assert!(validate(&req(200.0, vec![debt("A", 1000.0, f64::NAN, 50.0)])).is_err());
    }

    #[test]
    fn rejects_negative_min_payment() {
        assert!(validate(&req(200.0, vec![debt("A", 1000.0, 10.0, -10.0)])).is_err());
    }

    #[test]
    fn rejects_nan_min_payment() {
        assert!(validate(&req(200.0, vec![debt("A", 1000.0, 10.0, f64::NAN)])).is_err());
    }

    #[test]
    fn rejects_payment_below_sum_of_minimums() {
        assert!(validate(&req(50.0, vec![debt("A", 1000.0, 15.0, 100.0)])).is_err());
    }

    #[test]
    fn allows_zero_interest_rate() {
        assert!(validate(&req(200.0, vec![debt("A", 1000.0, 0.0, 50.0)])).is_ok());
    }

    #[test]
    fn allows_zero_min_payment() {
        assert!(validate(&req(200.0, vec![debt("A", 1000.0, 10.0, 0.0)])).is_ok());
    }

    #[test]
    fn allows_payment_exactly_at_minimum() {
        assert!(validate(&req(100.0, vec![debt("A", 1000.0, 15.0, 100.0)])).is_ok());
    }

    // --- HTTP integration tests (only for cases that need full handler wiring) ---

    #[tokio::test]
    async fn http_empty_debts_returns_400() {
        use serde_json::json;
        let res = post_json(app(), json!({ "monthly_payment": 500.0, "debts": [] })).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn http_payment_below_minimums_returns_400() {
        use serde_json::json;
        let body = json!({
            "monthly_payment": 50.0,
            "debts": [
                { "name": "Card A", "balance": 1000.0, "interest_rate": 15.0, "min_payment": 100.0 }
            ]
        });
        let res = post_json(app(), body).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn http_valid_request_passes_validation() {
        use serde_json::json;
        // Passes validation. PDF/GCS will fail downstream — we only check it's not a 400.
        let body = json!({
            "monthly_payment": 100.0,
            "debts": [
                { "name": "Card A", "balance": 1000.0, "interest_rate": 15.0, "min_payment": 100.0 }
            ]
        });
        let res = post_json(app(), body).await;
        assert_ne!(res.status(), StatusCode::BAD_REQUEST);
    }
}
