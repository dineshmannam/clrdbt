# scripts/

One-time operational scripts for clrdbt infrastructure.

## bootstrap.sh

Sets up the full MVP GCP infrastructure from scratch:
GCP project → Artifact Registry → GCS bucket → Secret Manager → CI/CD service account → Workload Identity Federation.

### Prerequisites

- `gcloud` CLI installed and authenticated (`gcloud auth login`)
- A GCP billing account ID
- Owner or Project Creator role on the GCP organization

### Usage

```bash
export PROJECT_ID=clrdbt-prod          # must be globally unique
export BILLING_ACCOUNT=XXXXXX-XXXXXX-XXXXXX
export GITHUB_REPO=dineshmannam/clrdbt  # for GitHub Actions Workload Identity
./scripts/bootstrap.sh
```

The script is idempotent — safe to re-run if something fails partway through.

### What it creates

| Resource | Purpose |
|---|---|
| GCP project | Container for all MVP resources |
| Artifact Registry repo | Stores Docker images built by CI |
| GCS bucket | Stores generated PDFs at `/pdfs/{id}.pdf` |
| Secret Manager secrets | `gcs-bucket-name`, `gcs-access-token`, `app-base-url` |
| CI/CD service account | Used by GitHub Actions to deploy — least-privilege only |
| Workload Identity Federation | Lets GitHub Actions authenticate without SA keys |

Run once per environment (prod, staging).
