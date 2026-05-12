use anyhow::Result;
use std::env;

const PDF_PREFIX: &str = "pdfs";

fn bucket() -> String {
    env::var("GCS_BUCKET_NAME").expect("GCS_BUCKET_NAME must be set")
}

fn access_token() -> Result<String> {
    Ok(env::var("GCS_ACCESS_TOKEN")
        .or_else(|_| env::var("GCS_SERVICE_ACCOUNT_KEY"))
        .unwrap_or_default())
}

/// Upload PDF bytes to GCS under the /pdfs/ prefix.
pub async fn upload_pdf(pdf_id: &str, pdf_bytes: Vec<u8>) -> Result<()> {
    let object_name = format!("{PDF_PREFIX}/{pdf_id}.pdf");
    upload_object(&object_name, pdf_bytes).await
}

/// Generate a signed download URL (15-minute expiry) for the PDF.
pub async fn signed_url(pdf_id: &str) -> Result<String> {
    let object_name = format!("{PDF_PREFIX}/{pdf_id}.pdf");
    generate_signed_url(&object_name).await
}

// ── Internal GCS helpers ──────────────────────────────────────────────────────

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

async fn generate_signed_url(object_name: &str) -> Result<String> {
    // MVP: plain GCS URL with access token. Replace with V4 signed URLs post-MVP.
    let bucket = bucket();
    let token = access_token()?;
    let url = format!(
        "https://storage.googleapis.com/storage/v1/b/{bucket}/o/{}?alt=media&access_token={token}",
        urlencoding::encode(object_name)
    );
    Ok(url)
}

mod urlencoding {
    pub fn encode(s: &str) -> String {
        s.chars()
            .flat_map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => vec![c],
                '/' => vec!['%', '2', 'F'],
                _ => {
                    let encoded = format!("%{:02X}", c as u32);
                    encoded.chars().collect()
                }
            })
            .collect()
    }
}
