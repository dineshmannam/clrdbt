# PDF Report Design Brief

## What is clrdbt?

**clrdbt.com** is a debt payoff plan generator. A user visits the site, enters their debts (credit cards, loans, etc.), tells the tool how much they can pay per month total, and within seconds gets a one-page PDF showing exactly when each debt will be paid off — and most importantly, the single date when they'll be completely debt-free.

That date is the product. Everything else on the page supports it.

The payoff method is called the **debt snowball**: debts are paid off smallest balance first. As each one clears, that freed-up money rolls into the next one. It's psychologically effective — users get quick wins early, which builds momentum.

The price is $9 (post-launch). The PDF is what they're paying for.

---

## The Emotional Job

This document needs to feel like a **commitment, not a spreadsheet**. The user is making a decision to get out of debt. The PDF is the artifact of that decision — something they print, sign with a pen, and hang somewhere visible (fridge, desk, bathroom mirror).

The signature line is literal. There is a line at the bottom for them to physically sign.

The tagline is: **"Sign it. Hang it. Own it."**

---

## What Goes on the Page

Here is every piece of data that will be populated into the template, with realistic examples:

### Header / Hero

- Label: `"Your Debt-Free Date"`
- The date: e.g. `June 2028` ← this is the hero. Largest element on the page.

### Debt Payoff Table

One row per debt, sorted smallest balance first:

| Debt Name | Starting Balance | Interest Rate | Monthly Minimum | Paid Off |
|---|---|---|---|---|
| Capital One Visa | $1,200.00 | 19.99% | $25.00 | August 2026 |
| Personal Loan | $4,500.00 | 11.50% | $95.00 | March 2027 |
| Student Loan | $8,500.00 | 6.80% | $150.00 | June 2028 |

- Minimum 1 debt, maximum ~10 debts. The layout must handle both without overflowing the page.
- The last debt in the table will always share the same payoff date as the hero date at the top.

### Summary Row

Below the table:

| Total Debt | Total Interest Paid | Monthly Payment |
|---|---|---|
| $14,200.00 | $3,841.22 | $450.00 |

### Footer

- Generated date: `May 2026` (small, subtle — for reference)
- Signature line: a horizontal rule with space to sign above it
- Tagline below the line: `Sign it. Hang it. Own it.`

---

## Design Rules

- **One page only.** US Letter size (8.5 × 11 inches). Content must never overflow — if someone has 10 debts, the table needs to compress to fit, not spill onto a second page.
- **Black and white safe.** The user might print this on a home printer. Any meaning conveyed through color must also work in greyscale.
- **The debt-free date is the hero.** It should be the first thing the eye lands on. Think large — like a poster headline, not a document title.
- **Minimal and clean.** This is not a bank statement. It should feel calm and purposeful. No clutter, no decoration for its own sake.
- **Printable.** Generous margins, readable font sizes, nothing that looks good on screen but bad on paper.

---

## Technical Constraints

- The template is HTML + CSS only — no JavaScript.
- Fonts must be embedded or web-safe (no Google Fonts or other external calls at render time).
- Rendered by headless Chromium at exactly 8.5×11in with 0.4in top/bottom margins and 0.5in left/right margins.
- Template variables use Jinja2 syntax: `{{ debt_free_date }}`, `{{ total_balance }}`, `{% for debt in debts %}`, etc.

---

## What to Deliver

A wireframe or mockup (any format) showing:

1. The layout at full page with 3–4 sample debts
2. How the table compresses with 8–10 debts (does the font size reduce? does the summary move up?)
3. The signature section at the bottom

Once the design is approved, the HTML/CSS gets dropped directly into the template file — so closer to real HTML in the wireframe means less translation work later.
