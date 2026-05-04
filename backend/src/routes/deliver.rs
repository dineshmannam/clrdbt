use axum::{extract::Query, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::storage;

#[derive(Deserialize)]
pub struct DeliverQuery {
    pub session_id: String,
}

#[derive(Serialize)]
pub struct DeliverResponse {
    pub download_url: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

pub async fn handle(
    Query(params): Query<DeliverQuery>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let session_id = &params.session_id;

    if session_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "Missing session_id.".into() }),
        ));
    }

    // Verify payment with Stripe
    match verify_stripe_payment(session_id).await {
        Ok(true) => {}
        Ok(false) => {
            return Err((
                StatusCode::PAYMENT_REQUIRED,
                Json(ErrorResponse { error: "Payment not completed.".into() }),
            ));
        }
        Err(e) => {
            error!("Stripe verification failed: {e}");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: "Payment verification failed.".into() }),
            ));
        }
    }

    // Move PDF from /pending/ to /delivered/
    if let Err(e) = storage::move_to_delivered(session_id).await {
        // May already be delivered (idempotent re-delivery)
        // Log but continue — try to return a signed URL anyway
        error!("Move to delivered failed (may already be delivered): {e}");
    }

    // Generate signed URL
    let url = match storage::signed_url(session_id).await {
        Ok(u) => u,
        Err(e) => {
            error!("Signed URL generation failed: {e}");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: "Download URL generation failed.".into() }),
            ));
        }
    };

    info!("Delivering PDF for session {session_id}");
    Ok((StatusCode::OK, Json(DeliverResponse { download_url: url })))
}

async fn verify_stripe_payment(session_id: &str) -> anyhow::Result<bool> {
    let secret_key = std::env::var("STRIPE_SECRET_KEY")?;
    let client = reqwest::Client::new();

    let url = format!("https://api.stripe.com/v1/checkout/sessions/{session_id}");
    let res = client
        .get(&url)
        .basic_auth(&secret_key, Option::<&str>::None)
        .send()
        .await?;

    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("Stripe API error: {body}");
    }

    let body: serde_json::Value = res.json().await?;
    let paid = body["payment_status"].as_str() == Some("paid");

    Ok(paid)
}
