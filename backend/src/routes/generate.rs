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

pub async fn handle(
    Json(req): Json<GenerateRequest>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    if req.debts.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "At least one debt is required.".into() }),
        ));
    }

    let total_min: f64 = req.debts.iter().map(|d| d.min_payment).sum();
    if req.monthly_payment < total_min {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!(
                    "Monthly payment must be at least ${:.2} (sum of minimums).",
                    total_min
                ),
            }),
        ));
    }

    let result = calculation::calculate(req.debts, req.monthly_payment);
    info!("Debt-free date calculated: {}", result.debt_free_date);

    let pdf_bytes = match pdf::render_pdf(&result).await {
        Ok(b) => b,
        Err(e) => {
            error!("PDF generation failed: {e}");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: "PDF generation failed.".into() }),
            ));
        }
    };

    let pdf_id = Uuid::new_v4().to_string();

    if let Err(e) = storage::upload_pdf(&pdf_id, pdf_bytes).await {
        error!("GCS upload failed: {e}");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Storage failed.".into() }),
        ));
    }

    let download_url = match storage::signed_url(&pdf_id).await {
        Ok(u) => u,
        Err(e) => {
            error!("Signed URL generation failed: {e}");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: "Download URL generation failed.".into() }),
            ));
        }
    };

    info!("PDF ready: {pdf_id}");
    Ok((StatusCode::OK, Json(GenerateResponse { download_url })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::{Request, StatusCode}, routing::post, Router};
    use serde_json::json;
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

    #[tokio::test]
    async fn empty_debts_returns_400() {
        let res = post_json(app(), json!({ "monthly_payment": 500.0, "debts": [] })).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn payment_below_minimums_returns_400() {
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
    async fn payment_exactly_at_minimum_passes_validation() {
        // Should pass validation (400 not returned), even if PDF/GCS fails downstream.
        // We check that it's NOT a 400 — downstream errors give 500.
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
