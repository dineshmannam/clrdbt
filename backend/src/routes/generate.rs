use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{calculation, pdf, storage};

#[derive(Deserialize)]
pub struct GenerateRequest {
    pub monthly_payment: f64,
    pub debts: Vec<calculation::DebtInput>,
}

#[derive(Serialize)]
pub struct GenerateResponse {
    pub checkout_url: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

pub async fn handle(
    Json(req): Json<GenerateRequest>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    // Basic validation
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

    // Run snowball calculation
    let result = calculation::calculate(req.debts, req.monthly_payment);
    info!("Debt-free date calculated: {}", result.debt_free_date);

    // Generate PDF
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

    // Create Stripe Checkout session to get a session_id
    let session = match create_stripe_session().await {
        Ok(s) => s,
        Err(e) => {
            error!("Stripe session creation failed: {e}");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: "Payment setup failed.".into() }),
            ));
        }
    };

    // Upload PDF to GCS /pending/
    if let Err(e) = storage::upload_pending(&session.id, pdf_bytes).await {
        error!("GCS upload failed: {e}");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: "Storage failed.".into() }),
        ));
    }

    Ok((StatusCode::OK, Json(GenerateResponse { checkout_url: session.url })))
}

struct StripeSession {
    id: String,
    url: String,
}

async fn create_stripe_session() -> anyhow::Result<StripeSession> {
    let secret_key = std::env::var("STRIPE_SECRET_KEY")?;
    let success_url = std::env::var("APP_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into())
        + "/success.html?session_id={CHECKOUT_SESSION_ID}";
    let cancel_url = std::env::var("APP_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into())
        + "/form.html";

    let client = reqwest::Client::new();
    let res = client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .basic_auth(&secret_key, Option::<&str>::None)
        .form(&[
            ("payment_method_types[]", "card"),
            ("mode", "payment"),
            ("line_items[0][price_data][currency]", "usd"),
            ("line_items[0][price_data][product_data][name]", "clrdbt — Debt Payoff Plan"),
            ("line_items[0][price_data][unit_amount]", "900"), // $9.00 in cents
            ("line_items[0][quantity]", "1"),
            ("success_url", &success_url),
            ("cancel_url", &cancel_url),
        ])
        .send()
        .await?;

    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("Stripe error: {body}");
    }

    let body: serde_json::Value = res.json().await?;
    let id = body["id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing session id"))?.to_string();
    let url = body["url"].as_str().ok_or_else(|| anyhow::anyhow!("Missing checkout url"))?.to_string();

    Ok(StripeSession { id, url })
}
