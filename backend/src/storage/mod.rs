use anyhow::Result;
use std::env;

const PENDING_PREFIX: &str = "pending";
const DELIVERED_PREFIX: &str = "delivered";

fn bucket() -> String {
    env::var("GCS_BUCKET_NAME").expect("GCS_BUCKET_NAME must be set")
}

/// Upload PDF bytes to GCS under the /pending/ prefix.
pub async fn upload_pending(session_id: &str, pdf_bytes: Vec<u8>) -> Result<()> {
    let object_name = format!("{PENDING_PREFIX}/{session_id}.pdf");
    upload_object(&object_name, pdf_bytes).await
}

/// Move a PDF from /pending/ to /delivered/ by copying then deleting.
pub async fn move_to_delivered(session_id: &str) -> Result<()> {
    let src = format!("{PENDING_PREFIX}/{session_id}.pdf");
    let dst = format!("{DELIVERED_PREFIX}/{session_id}.pdf");
    copy_object(&src, &dst).await?;
    delete_object(&src).await?;
    Ok(())
}

/// Generate a signed download URL (15-minute expiry) for the delivered PDF.
pub async fn signed_url(session_id: &str) -> Result<String> {
    let object_name = format!("{DELIVERED_PREFIX}/{session_id}.pdf");
    generate_signed_url(&object_name, 15 * 60).await
}

// ── Internal GCS helpers ──────────────────────────────────────────────────────

fn access_token() -> Result<String> {
    // In production, use workload identity or a service account key.
    // For now, read from environment.
    Ok(env::var("GCS_ACCESS_TOKEN")
        .or_else(|_| env::var("GCS_SERVICE_ACCOUNT_KEY"))
        .unwrap_or_default())
}

async fn upload_object(object_name: &str, data: Vec<u8>) -> Result<()> {
    let bucket = bucket();
    let token = access_token()?;
    let url = format!(
        "https://storage.googleapis.com/upload/storage/v1/b/{bucket}/o?uploadType=media&name={object_name}"
    );

    let client = reqwest::Client::new();
    let res = client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/pdf")
        .body(data)
        .send()
        .await?;

    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("GCS upload failed: {body}");
    }

    Ok(())
}

async fn copy_object(src: &str, dst: &str) -> Result<()> {
    let bucket = bucket();
    let token = access_token()?;
    let url = format!(
        "https://storage.googleapis.com/storage/v1/b/{bucket}/o/{}/copyTo/b/{bucket}/o/{}",
        urlencoding::encode(src),
        urlencoding::encode(dst)
    );

    let client = reqwest::Client::new();
    let res = client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await?;

    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("GCS copy failed: {body}");
    }

    Ok(())
}

async fn delete_object(object_name: &str) -> Result<()> {
    let bucket = bucket();
    let token = access_token()?;
    let url = format!(
        "https://storage.googleapis.com/storage/v1/b/{bucket}/o/{}",
        urlencoding::encode(object_name)
    );

    let client = reqwest::Client::new();
    let res = client
        .delete(&url)
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await?;

    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("GCS delete failed: {body}");
    }

    Ok(())
}

async fn generate_signed_url(object_name: &str, _expires_in_secs: u64) -> Result<String> {
    // For MVP, return a plain GCS URL with a token.
    // In production, replace with V4 signed URL using service account key.
    let bucket = bucket();
    let token = access_token()?;
    let url = format!(
        "https://storage.googleapis.com/storage/v1/b/{bucket}/o/{}?alt=media&access_token={token}",
        urlencoding::encode(object_name)
    );
    Ok(url)
}

// Tiny helper — avoids pulling in percent-encoding crate
mod urlencoding {
    pub fn encode(s: &str) -> String {
        s.chars()
            .flat_map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                    vec![c]
                }
                '/' => vec!['%', '2', 'F'],
                _ => {
                    let encoded = format!("%{:02X}", c as u32);
                    encoded.chars().collect()
                }
            })
            .collect()
    }
}
