#!/usr/bin/env bash
# Bootstrap GCP infrastructure for clrdbt MVP (Cloud Run).
# Idempotent — safe to re-run if something fails partway through.
#
# Prerequisites:
#   - gcloud CLI installed and authenticated (gcloud auth login)
#   - BILLING_ACCOUNT set (see scripts/README.md)
#   - Owner or Project Creator role on the GCP organization
#
# Usage:
#   export PROJECT_ID=clrdbt-prod
#   export BILLING_ACCOUNT=XXXXXX-XXXXXX-XXXXXX
#   export GITHUB_REPO=your-org/clrdbt   # optional, for Workload Identity Federation
#   ./scripts/bootstrap.sh

set -euo pipefail

# ── Configuration ───────────────────────────────────────────────────────────────

PROJECT_ID="${PROJECT_ID:-clrdbt-prod}"
BILLING_ACCOUNT="${BILLING_ACCOUNT:?Set BILLING_ACCOUNT to your GCP billing account ID}"
REGION="${REGION:-us-central1}"
AR_REPO="${AR_REPO:-clrdbt}"
GCS_BUCKET="${GCS_BUCKET:-${PROJECT_ID}-pdfs}"
SA_NAME="${SA_NAME:-clrdbt-cicd}"
SA_EMAIL="${SA_NAME}@${PROJECT_ID}.iam.gserviceaccount.com"
GITHUB_REPO="${GITHUB_REPO:-}"  # e.g. dineshmannam/clrdbt — required for WIF setup

APIS=(
  run.googleapis.com
  artifactregistry.googleapis.com
  storage.googleapis.com
  secretmanager.googleapis.com
  iam.googleapis.com
  iamcredentials.googleapis.com
  cloudresourcemanager.googleapis.com
  sts.googleapis.com
)

# ── Helpers ───────────────────────────────────────────────────────────────────

info()  { echo "==> $*"; }
warn()  { echo "WARNING: $*" >&2; }
die()   { echo "ERROR: $*" >&2; exit 1; }

gcloud_project_exists() {
  gcloud projects describe "$PROJECT_ID" &>/dev/null
}

# ── 1. Project + billing ────────────────────────────────────────────────────────

info "Project: $PROJECT_ID  Region: $REGION"

if gcloud_project_exists; then
  info "Project already exists — skipping creation"
else
  info "Creating project $PROJECT_ID"
  gcloud projects create "$PROJECT_ID" --name="clrdbt"
fi

gcloud config set project "$PROJECT_ID"

info "Linking billing account"
gcloud billing projects link "$PROJECT_ID" --billing-account="$BILLING_ACCOUNT" 2>/dev/null \
  || info "Billing already linked (or link failed — check manually)"

# ── 2. Enable APIs ─────────────────────────────────────────────────────────────

info "Enabling required APIs"
for api in "${APIS[@]}"; do
  gcloud services enable "$api" --project="$PROJECT_ID" --quiet
done

# ── 3. Artifact Registry ───────────────────────────────────────────────────────

info "Creating Artifact Registry repo: $AR_REPO"
if gcloud artifacts repositories describe "$AR_REPO" \
    --location="$REGION" --project="$PROJECT_ID" &>/dev/null; then
  info "Artifact Registry repo already exists"
else
  gcloud artifacts repositories create "$AR_REPO" \
    --repository-format=docker \
    --location="$REGION" \
    --description="clrdbt Docker images" \
    --project="$PROJECT_ID"
fi

# ── 4. GCS bucket ──────────────────────────────────────────────────────────────

info "Creating GCS bucket: $GCS_BUCKET"
if gcloud storage buckets describe "gs://${GCS_BUCKET}" --project="$PROJECT_ID" &>/dev/null; then
  info "GCS bucket already exists"
else
  gcloud storage buckets create "gs://${GCS_BUCKET}" \
    --location="$REGION" \
    --uniform-bucket-level-access \
    --project="$PROJECT_ID"
fi

# ── 5. CI/CD service account ──────────────────────────────────────────────────

info "Creating CI/CD service account: $SA_EMAIL"
if gcloud iam service-accounts describe "$SA_EMAIL" --project="$PROJECT_ID" &>/dev/null; then
  info "Service account already exists"
else
  gcloud iam service-accounts create "$SA_NAME" \
    --display-name="clrdbt CI/CD" \
    --project="$PROJECT_ID"
fi

info "Binding IAM roles to CI/CD service account"
ROLES=(
  roles/artifactregistry.writer
  roles/run.admin
  roles/iam.serviceAccountUser
  roles/storage.objectAdmin
  roles/secretmanager.secretAccessor
)
for role in "${ROLES[@]}"; do
  gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:${SA_EMAIL}" \
    --role="$role" \
    --condition=None \
    --quiet &>/dev/null || true
done

# ── 6. Secret Manager (placeholder secrets) ───────────────────────────────────

info "Creating Secret Manager secrets (placeholder values)"
SECRETS=(
  gcs-bucket-name
  gcs-access-token
  app-base-url
)
for secret in "${SECRETS[@]}"; do
  if gcloud secrets describe "$secret" --project="$PROJECT_ID" &>/dev/null; then
    info "Secret '$secret' already exists"
  else
    echo -n "PLACEHOLDER" | gcloud secrets create "$secret" \
      --data-file=- \
      --replication-policy=automatic \
      --project="$PROJECT_ID"
    info "Created secret '$secret' — update the value manually (see below)"
  fi
done

# Set gcs-bucket-name to the actual bucket name if still placeholder
CURRENT_BUCKET_SECRET=$(gcloud secrets versions access latest --secret=gcs-bucket-name --project="$PROJECT_ID" 2>/dev/null || echo "")
if [[ "$CURRENT_BUCKET_SECRET" == "PLACEHOLDER" ]]; then
  echo -n "$GCS_BUCKET" | gcloud secrets versions add gcs-bucket-name --data-file=- --project="$PROJECT_ID"
  info "Set gcs-bucket-name secret to $GCS_BUCKET"
fi

# ── 7. Workload Identity Federation (GitHub Actions) ──────────────────────────

WIF_POOL="github"
WIF_PROVIDER="github"
WIF_PROVIDER_FULL="projects/$(gcloud projects describe "$PROJECT_ID" --format='value(projectNumber)')/locations/global/workloadIdentityPools/${WIF_POOL}/providers/${WIF_PROVIDER}"

if [[ -n "$GITHUB_REPO" ]]; then
  info "Setting up Workload Identity Federation for GitHub repo: $GITHUB_REPO"

  if ! gcloud iam workload-identity-pools describe "$WIF_POOL" \
      --location=global --project="$PROJECT_ID" &>/dev/null; then
    gcloud iam workload-identity-pools create "$WIF_POOL" \
      --location=global \
      --display-name="GitHub Actions" \
      --project="$PROJECT_ID"
  fi

  if ! gcloud iam workload-identity-pools providers describe "$WIF_PROVIDER" \
      --workload-identity-pool="$WIF_POOL" --location=global --project="$PROJECT_ID" &>/dev/null; then
    gcloud iam workload-identity-pools providers create-oidc "$WIF_PROVIDER" \
      --workload-identity-pool="$WIF_POOL" \
      --location=global \
      --issuer-uri="https://token.actions.githubusercontent.com" \
      --attribute-mapping="google.subject=assertion.sub,attribute.repository=assertion.repository" \
      --attribute-condition="assertion.repository=='${GITHUB_REPO}'" \
      --project="$PROJECT_ID"
  fi

  gcloud iam service-accounts add-iam-policy-binding "$SA_EMAIL" \
    --role="roles/iam.workloadIdentityUser" \
    --member="principalSet://iam.googleapis.com/projects/$(gcloud projects describe "$PROJECT_ID" --format='value(projectNumber)')/locations/global/workloadIdentityPools/${WIF_POOL}/attribute.repository/${GITHUB_REPO}" \
    --project="$PROJECT_ID" \
    --quiet &>/dev/null || true

  info "Workload Identity Federation configured"
else
  warn "GITHUB_REPO not set — skipping Workload Identity Federation setup"
  warn "Set GITHUB_REPO=dineshmannam/clrdbt and re-run to wire GitHub Actions auth"
  WIF_PROVIDER_FULL="<run again with GITHUB_REPO set>"
fi

# ── 8. Summary ────────────────────────────────────────────────────────────────

PROJECT_NUMBER=$(gcloud projects describe "$PROJECT_ID" --format='value(projectNumber)')
IMAGE_BASE="${REGION}-docker.pkg.dev/${PROJECT_ID}/${AR_REPO}/clrdbt"

cat <<EOF

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  clrdbt GCP bootstrap complete
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Project ID:       $PROJECT_ID
Project number:   $PROJECT_NUMBER
Region:           $REGION
GCS bucket:       gs://${GCS_BUCKET}
Artifact Registry: ${IMAGE_BASE}
CI/CD SA:         $SA_EMAIL

── Manual steps ────────────────────────────────────────────────────────────────

1. Set Secret Manager values:

   echo -n "${GCS_BUCKET}" | gcloud secrets versions add gcs-bucket-name --data-file=-
   echo -n "YOUR_GCS_ACCESS_TOKEN" | gcloud secrets versions add gcs-access-token --data-file=-
   echo -n "https://clrdbt.com" | gcloud secrets versions add app-base-url --data-file=-

   For gcs-access-token: create a service account with roles/storage.objectAdmin,
   then run: gcloud auth print-access-token (for dev) or use a SA key / WIF token
   in production. Cloud Run should use the runtime SA with storage.objectAdmin instead
   of a static token — update storage/mod.rs post-MVP.

2. Add these GitHub Actions secrets (Settings → Secrets → Actions):

   GCP_PROJECT_ID              = $PROJECT_ID
   GCP_SERVICE_ACCOUNT         = $SA_EMAIL
   GCP_WORKLOAD_IDENTITY_PROVIDER = $WIF_PROVIDER_FULL

3. Deploy manually (first time, before CD is wired):

   gcloud auth configure-docker ${REGION}-docker.pkg.dev
   docker build -t ${IMAGE_BASE}:latest .
   docker push ${IMAGE_BASE}:latest
   gcloud run deploy clrdbt \\
     --image ${IMAGE_BASE}:latest \\
     --region $REGION \\
     --allow-unauthenticated \\
     --set-secrets GCS_BUCKET_NAME=gcs-bucket-name:latest,GCS_ACCESS_TOKEN=gcs-access-token:latest \\
     --set-env-vars APP_BASE_URL=https://clrdbt.com \\
     --memory 1Gi \\
     --project $PROJECT_ID

4. Map custom domain clrdbt.com in Cloud Run console after first deploy.

5. Merge dev → main to trigger CD on future deploys.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
EOF
