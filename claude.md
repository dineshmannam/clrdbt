# CLAUDE.md — clrdbt

## What This Project Is

**clrdbt.com** — a debt payoff plan generator. Users input their debts, pay $9, and receive a one-page PDF showing the snowball payoff order and their exact debt-free date. The date is the product. The math is the credibility.

**Core differentiator:** The PDF has a signature line. Users sign it and hang it somewhere visible. "Sign it. Hang it. Own it."

**Headline:** "Your debt has an end date. Find it."

---

## Architecture

```
User → Frontend (HTML + Tailwind)
     → Rust/Axum Backend
          → Snowball Calculation
          → HTML Template → Chromiumoxide → PDF
          → Google Cloud Storage (GCS)
          → Stripe Checkout
          → Resend (transactional email)
     → Cloud Run (MVP) → GKE post-MVP
     → GitHub Actions (CI/CD)
```

### Stack

| Layer | Technology |
|---|---|
| Frontend | HTML + Tailwind CSS |
| Backend | Rust + Axum |
| PDF Generation | HTML template → Chromiumoxide |
| Storage | Google Cloud Storage |
| Payments | Stripe |
| Transactional Email | Resend |
| Container Registry | Google Artifact Registry |
| Orchestration (MVP) | Cloud Run |
| Orchestration (post-MVP) | GKE (Kubernetes) |
| CI/CD | GitHub Actions |
| Domain | clrdbt.com |

---

## Repository Structure

```
clrdbt/
├── frontend/          # HTML + Tailwind pages
│   ├── index.html     # Landing page
│   ├── form.html      # Debt input form
│   └── success.html   # Thank you + download trigger
├── backend/           # Rust/Axum service
│   ├── src/
│   │   ├── main.rs
│   │   ├── routes/
│   │   ├── calculation/   # Snowball logic
│   │   ├── pdf/           # HTML template + Chromiumoxide
│   │   └── storage/       # GCS operations
│   ├── templates/         # HTML template for PDF
│   └── Cargo.toml
├── k8s/               # Kubernetes manifests
│   ├── deployment.yaml
│   ├── service.yaml
│   └── ingress.yaml
├── .github/
│   └── workflows/
│       ├── ci.yml     # Runs on every push to dev
│       └── cd.yml     # Runs on merge to main
├── Dockerfile
└── CLAUDE.md
```

---

## Branch Strategy

```
main    ← production only, protected, no direct commits
└── dev ← daily work, CI runs on every push
    ├── feature/xxx   ← short-lived, merge back to dev fast
    └── fix/xxx       ← bug fixes, merge back to dev fast
```

**Branch naming:** `feature/snowball-calculation`, `fix/gcs-signed-url`
**Rule:** Never commit broken code. Every commit leaves the app in a working state.
**Merges:** PRs required into both `dev` and `main`. CI must pass before merge.

---

## User Flow

```
1. Landing page (index.html)
2. Debt input form (form.html) — no email field, Stripe collects it at checkout
3. POST /api/generate → backend calculates, generates PDF, saves to GCS /pending/
4. Stripe Checkout session created → user redirected to Stripe
5. Payment complete → Stripe redirects to success.html?session_id=xxx
6. success.html calls GET /api/deliver?session_id=xxx
7. Backend verifies payment → moves PDF from /pending/ to /delivered/
8. Returns signed GCS URL (15 min expiry) → auto-download triggers in browser
9. Resend sends one transactional email with the same signed URL
   (Stripe also sends its own receipt automatically — we don't build that)
```

**Abandoned payments:** GCS lifecycle policy deletes `/pending/` files after 24 hours. No code required.

---

## GCS Bucket Structure

```
/pending/{session_id}.pdf    ← generated, not yet paid
/delivered/{session_id}.pdf  ← payment confirmed
```

Lifecycle rule: `/pending/` prefix → delete after 24 hours.

---

## Snowball Calculation Logic

1. User inputs: debt name, balance, interest rate, minimum payment (for each debt)
2. User inputs: total monthly payment amount
3. Sort debts by **balance ascending** (snowball — smallest first)
4. Apply minimum payments to all debts
5. Apply remaining money to smallest debt until paid off
6. Roll that payment into the next debt
7. Calculate exact payoff date for each debt
8. Final payoff date = the hero element of the PDF

---

## PDF Design Rules

- **The debt-free date is the hero.** Large, bold, centered at top. It should hit before anything else is read.
- Layout order:
  1. "Your Debt-Free Date" label (small)
  2. **MONTH DD, YYYY** (massive, bold)
  3. Payoff order table (normal size)
  4. Monthly breakdown summary
  5. Signature line at bottom: `Sign it. Hang it. Own it.`
- One page only. No exceptions.
- Clean, minimal, printable. Black and white safe.

---

## Stripe Integration

- Use Stripe Checkout (hosted page) — do not build custom payment UI
- No email field on the form — Stripe collects email at checkout
- Stripe automatically sends a receipt/invoice — we build nothing for this
- Store `session_id` as the key linking PDF to payment
- Verify payment server-side via Stripe API before issuing signed URL
- Webhook endpoint: `POST /api/webhook/stripe` — handle `checkout.session.completed`
- **Warning:** Stripe signature verification requires raw request body. Do not parse body before verifying signature.

---

## Email Delivery

- **Service:** Resend — transactional only
- **Trigger:** After payment confirmed, send one email with the signed GCS URL
- **Content:** PDF download link, nothing else
- **Email address source:** Retrieved from the Stripe session (`customer_details.email`)
- **Privacy:** Email used for delivery only. Not stored long-term. No marketing. No list building.
- **Privacy promise (use on landing page near buy button):**
  > "We don't store your email, sell your data, or send you anything except your plan."

---

## Data Model

Key backend struct for a submission:

```rust
struct DebtSubmission {
    debts: Vec<Debt>,
    monthly_payment: f64,
    email: String,    // delivery only, not stored
    session_id: String,
}
```

---

## Pricing

- **$9 flat.** No tiers. No subscriptions. No discounts.

---

## Development Philosophy

- This is a learning project for: Rust, Kubernetes, GCP, CI/CD pipelines, Git branching
- MVP first. Ship working software. Add features on top.
- Question every dependency. Every added library is a maintenance burden.
- Small, frequent commits. No large PRs.
- MVP runs on Cloud Run. GKE and full Kubernetes come post-MVP once the product is live.

---

## Environment Variables (never commit these)

```
STRIPE_SECRET_KEY
STRIPE_WEBHOOK_SECRET
GCS_BUCKET_NAME
GCS_ACCESS_TOKEN
RESEND_API_KEY
APP_BASE_URL
```

Use Google Secret Manager in production (Cloud Run reads secrets directly). Use `.env` locally (already in `.gitignore`).

---

## Key Decisions Log

| Decision | Choice | Reason |
|---|---|---|
| PDF delivery | Server-side, GCS | Enables re-delivery, consistent output |
| Payoff method | Snowball | Quick wins, psychological momentum |
| Payment | Stripe Checkout | No custom UI, battle-tested |
| Frontend framework | None (HTML + Tailwind) | 4 static pages, no state complexity |
| Backend language | Rust + Axum | Learning goal, performance |
| PDF rendering | Chromiumoxide | Design flexibility via HTML templates |
| Infrastructure (MVP) | Cloud Run | No cluster overhead, scales to zero, fast to ship |
| Infrastructure (post-MVP) | GKE | Learning goal for Kubernetes, after product is live |
| Pricing | $9 one-time | Trust signal, no subscription complexity |
| Monthly income input | Excluded for MVP | Focus on ideal payoff path |
| Email field on form | Excluded | Stripe collects email at checkout — no duplication |
| Invoice | Stripe built-in | Stripe sends receipt automatically — nothing to build |
| Transactional email | Resend | Simple API, one email per transaction, no marketing |
| Email data retention | Not stored | Privacy promise — delivery only, no list building |
