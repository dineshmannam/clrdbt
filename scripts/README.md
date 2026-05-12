# scripts/

One-time operational scripts for clrdbt infrastructure.

## bootstrap.sh *(coming — issue #4)*

Sets up the full MVP GCP infrastructure from scratch:
GCP project → Artifact Registry → GCS bucket → Cloud Run → Secret Manager → CI/CD service account.

**Prerequisites:**
- `gcloud` CLI installed and authenticated (`gcloud auth login`)
- A GCP billing account ID
- Owner or Project Creator role on the GCP organization

Run once per environment (prod, staging).
