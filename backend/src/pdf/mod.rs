use anyhow::Result;
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use minijinja::{Environment, Value};

use crate::calculation::SnowballResult;

pub async fn render_pdf(result: &SnowballResult) -> Result<Vec<u8>> {
    let html = render_html(result)?;
    html_to_pdf(&html).await
}

fn render_html(result: &SnowballResult) -> Result<String> {
    let template_src = include_str!("../../templates/report.html");

    let mut env = Environment::new();
    env.add_template("report", template_src)?;
    let tmpl = env.get_template("report")?;

    let ctx = minijinja::context! {
        debt_free_date => &result.debt_free_date,
        total_months => result.total_months,
        total_balance => &result.total_balance,
        total_interest => &result.total_interest,
        monthly_payment => &result.monthly_payment,
        generated_date => &result.generated_date,
        debts => Value::from_serialize(&result.debts),
    };

    Ok(tmpl.render(ctx)?)
}

async fn html_to_pdf(html: &str) -> Result<Vec<u8>> {
    let (mut browser, mut handler) = Browser::launch(
        BrowserConfig::builder()
            .no_sandbox()
            .build()
            .map_err(|e| anyhow::anyhow!("Browser config error: {e}"))?,
    )
    .await?;

    let handler_task = tokio::spawn(async move {
        loop {
            if handler.next().await.is_none() {
                break;
            }
        }
    });

    let page = browser.new_page("about:blank").await?;

    page.set_content(html).await?;

    let pdf_opts = chromiumoxide::page::PrintToPdfParams {
        print_background: Some(true),
        paper_width: Some(8.5),
        paper_height: Some(11.0),
        margin_top: Some(0.4),
        margin_bottom: Some(0.4),
        margin_left: Some(0.5),
        margin_right: Some(0.5),
        ..Default::default()
    };

    let pdf_bytes = page.pdf(pdf_opts).await?;

    browser.close().await?;
    handler_task.abort();

    Ok(pdf_bytes)
}
